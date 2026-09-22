# 0.8.8 附件接收修正

## 問題與修正

0.8.7 將刪除操作移到最近對話列，但 `ui/work.js` 的接收鎖定仍存取已不存在的 `delete-chat`。選完合法附件後，`fileBatchBusy` 已設為 true，而畫面更新在 try/finally 外拋出例外，導致 `file_begin` 尚未送出且接收狀態無法恢復。這與缺少附件 POST 請求一致，不能據此認定伺服器未回 job_id。

移除失效引用，依目前附件接收與主程式狀態統一更新送出、新對話、歷史及對話列操作按鈕。初次重繪移入 try/finally，成功或失敗後立即清除接收鎖定，不依賴下一次狀態推播；原生上傳與網站契約維持原狀。

## 驗證方式

先新增 WebView2 回歸案例，使用已有訊息的對話及大於單一區塊的合成 File（`.pdf` 檔名，內容只驗證傳輸位元組，不測試 PDF 轉檔），從選檔 change 處理器進入 `addFiles`，只模擬原生 ACK；檢查 begin、兩次 chunk、finish 的順序及每個 byte。另檢查接收期間禁止切換／刪除對話，成功後恢復控制項、原生拒絕區塊後 abort，以及同一檔案可再次選取。

修正前執行 `Build.ps1 -ValidateOnly`，81 項 Rust 測試通過，但新增的合法選檔案例確實重現 `TypeError: Cannot set properties of null (setting 'disabled')`；未覆寫原交付 EXE。

原生既有整合測試使用 loopback 模擬網站，驗證預約附件、PUT 原始 bytes、查詢處理狀態、取得附件 token 與聊天，不連公司服務。

正式建置使用 `scripts/Build.ps1 -EmptyCargoCache`：v142 x64 實際編譯器探針、fmt、Clippy、Rust 測試、空快取 frozen release、WebView2 自檢、EXE 更新清單簽章與 SHA256。依賴仍由 Cargo.lock、vendor、.cargo/config.toml 提供。

2026-09-22 修正後完整建置通過：MSVC 14.29.30133、`_MSC_FULL_VER=192930159` x64，fmt、Clippy、81 項 Rust 測試、空快取 frozen release、WebView2 自檢及 EXE 清單驗證皆成功。新增案例在修正前失敗、修正後通過。

成品版本 `0.8.8.0`，大小 `4,661,248` bytes，SHA256：`095cde161bfb3331d7a9a8329c63a6995806ad01be723224ec1ee49c6071f8da`。

## 交付界線

使用者已回報 0.8.8 大致正常，追加製作同版 NSIS 並推送 Git。完整建置命令為 `scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller`，交付 EXE、Setup、兩份更新清單與驗證記錄；離線 ZIP 不重製。本次未修改 VNC，不重做 Viewer 連線測試。

NSIS 測試使用 v142 編譯的兩代原生測試程式，檢查固定路徑、捷徑與登錄、等待舊程序退出、替換及重新啟動、檔案鎖定時保留舊版、解除安裝保留未知檔案。另以合成 `machines.json`／`user_config.json` 驗證更新及解除安裝不改寫設定；不使用真實機台密碼。

同一套測試會安裝真正交付的 Setup 至預先確認未使用的 `C:\largan\LM_AI`，比對安裝後 EXE 的 SHA256 並執行 WebView2 自檢，再解除安裝並確認清理完整。公司防毒、目錄寫入權限及缺少 WebView2 的全新電腦仍需公司環境驗證。

2026-09-22 NSIS 完整建置與上述安裝／更新／重啟／占用失敗／解除安裝測試全部通過，`machines.json` 及 `user_config.json` 原內容保留。真正交付包安裝後的 WebView2 自檢亦通過；測試產品目錄、登錄與捷徑已清理。編譯工具仍為 MSVC 14.29.30133 x64，81 項 Rust 測試及空快取 frozen 建置再次通過。

第一版完整安裝包版本 `0.8.8.0`，大小 `217,573,495` bytes，SHA256：`f14e4b51bad516c2cb6fd8c1454bac0ce9f6eb624663c749188b325238006686`。此包已由下方精簡版取代，僅保留此段作為歷史建置記錄。

## WebView2 拆包（歷史交付，已由下方單純安裝版取代）

依使用者指示，NSIS 不再嵌入、下載或自動執行 WebView2。獨立 Microsoft x64 Evergreen 安裝程式仍保存在 Git 的 `dist/`，供公司自行提供下載；本輪重新確認 SHA256 與 `WEBVIEW2-OFFLINE.md` 一致，Microsoft Authenticode 簽章有效。精簡包本身的建置不依賴該獨立 Runtime 檔案。

安裝初始化先檢查 HKCU／HKLM 的 Evergreen 版本，空值或 `0.0.0.0` 視為未安裝；缺少時以原生視窗提示取得公司提供的安裝程式，退出碼 2，尚未建立產品目錄或替換任何既有檔案。保留一般使用者安裝及固定路徑。

2026-09-22 再次執行 `Build.ps1 -EmptyCargoCache -IncludeInstaller` 全部通過：實際 MSVC v142 x64 探針、fmt、Clippy、81 項 Rust 測試、空 Cargo 快取 frozen release、WebView2 自檢與更新清單原生驗證。NSIS 測試新增空值／零版本的全新安裝拒絕與既有安裝保留，以及 HKCU 有效／HKCU 零值回退 HKLM 有效兩種成功案例；只在測試包注入讀值，不移除開發機 Runtime 或改動其登錄。這不等同在全新無 Runtime 電腦驗收；該環境仍需公司實測。

正式精簡 Setup 已實際安裝、自檢及解除安裝；更新／重啟／鎖定失敗／未知檔案及 VNC 設定保留測試通過。新版 Setup 為 `1,854,170` bytes（約 1.85 MB），SHA256：`3cd25e25767e41e102efe6d69685646737be5e8e634e4f05dfefba8229cd6171`。此為前次精簡包的歷史校驗資訊。版本仍為 0.8.8.0，主程式 EXE 與本文件上方 SHA256 相同；歷史離線 ZIP 未重製。

## 單純安裝與撤下 SFX（目前交付）

使用者要求 NSIS 單純安裝，並撤下被 Chrome 阻擋的 SFX 測試包。刪除最新 Git 的 SFX EXE、製作／測試腳本、設定與測試文件，不改寫歷史提交。NSIS 移除 Evergreen 登錄檢查與缺少 Runtime 時的安裝阻擋，也移除一般安裝完成頁的「開啟 LM_AI」動作。保留固定路徑、捷徑、解除安裝及既有設定；App 內取得同意後的更新等待／重新啟動契約維持原樣。

主程式未修改。`src/webview.rs` 原有的 WebView2 初始化失敗訊息會引導安裝 Runtime；正式執行由原生錯誤視窗顯示，自檢模式則寫入 stderr 並退出 1。不是在 NSIS 中啟動主程式檢查，也不新增另一套預先偵測。

使用者回報趨勢警報目標為 `msedge.exe`。核對原 NSIS 後，其 Runtime 檢查只讀登錄、不呼叫 msedge.exe；未取得完整防毒紀錄，不能認定偵測就是警報原因，也不能保證移除檢查能解除警報。Microsoft 說明正式 WebView2 應用程式使用 Runtime，與 Edge 穩定版瀏覽器不同；Windows 11 通常包含 Runtime，少數電腦仍可能缺少。參考 [Microsoft 部署說明](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)。

新增安裝測試使用 `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` 指向不存在的資料夾，只限測試子程序，不修改系統環境、登錄或本機 Runtime。真正 Setup 必須仍可安裝，已安裝主程式的自檢則必須退出 1 並帶有原有 Runtime 補裝提示；移除該子程序覆寫後，同一份 EXE 必須通過 WebView2 自檢。此為 Runtime 無法載入的模擬，不能代替公司無 Runtime 電腦或趨勢防毒驗收。

2026-09-22 最後一輪 `Build.ps1 -EmptyCargoCache -IncludeInstaller` 完整通過：MSVC 14.29.30133、實際 `_MSC_FULL_VER=192930159` x64、fmt、Clippy、81 項 Rust 測試、空快取 frozen release、原生簽署清單驗證，以及完整安裝／更新／重啟／鎖定失敗／設定保留／解除安裝。上述 Runtime 無法載入模擬及正常啟動自檢均通過，測試產品目錄、登錄與捷徑已清理。

驗證過程曾有兩輪安裝後的正常介面自檢回傳失敗：第一次舊測試腳本未收集 stderr；補上錯誤輸出後，第二次顯示 `Error: stream preserves reading position`。未修改主程式、放寬斷言或略過測試，後續完整重跑通過。這項既有自檢有間歇性失敗，原因尚未查明，不宣稱本次已修復串流捲動；保留此記錄供後續追蹤。

目前 Setup 大小 `1,854,147` bytes，SHA256：`45093ef5f31d45cf8ff0b84808d0491d88f005f35f82d2a9f09cc1d27f89be9d`。版本仍為 0.8.8.0，主程式 EXE 的 SHA256 仍為本文件上方的 `095cde…71f8da`。部署時須一起替換 Setup 與最新 `dist/update-manifest.json`，避免舊下載快取與新清單不符；不更新歷史離線 ZIP。
