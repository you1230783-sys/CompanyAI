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

## WebView2 拆包（目前交付）

依使用者指示，NSIS 不再嵌入、下載或自動執行 WebView2。獨立 Microsoft x64 Evergreen 安裝程式仍保存在 Git 的 `dist/`，供公司自行提供下載；本輪重新確認 SHA256 與 `WEBVIEW2-OFFLINE.md` 一致，Microsoft Authenticode 簽章有效。精簡包本身的建置不依賴該獨立 Runtime 檔案。

安裝初始化先檢查 HKCU／HKLM 的 Evergreen 版本，空值或 `0.0.0.0` 視為未安裝；缺少時以原生視窗提示取得公司提供的安裝程式，退出碼 2，尚未建立產品目錄或替換任何既有檔案。保留一般使用者安裝及固定路徑。

2026-09-22 再次執行 `Build.ps1 -EmptyCargoCache -IncludeInstaller` 全部通過：實際 MSVC v142 x64 探針、fmt、Clippy、81 項 Rust 測試、空 Cargo 快取 frozen release、WebView2 自檢與更新清單原生驗證。NSIS 測試新增空值／零版本的全新安裝拒絕與既有安裝保留，以及 HKCU 有效／HKCU 零值回退 HKLM 有效兩種成功案例；只在測試包注入讀值，不移除開發機 Runtime 或改動其登錄。這不等同在全新無 Runtime 電腦驗收；該環境仍需公司實測。

正式精簡 Setup 已實際安裝、自檢及解除安裝；更新／重啟／鎖定失敗／未知檔案及 VNC 設定保留測試通過。新版 Setup 為 `1,854,170` bytes（約 1.85 MB），SHA256：`3cd25e25767e41e102efe6d69685646737be5e8e634e4f05dfefba8229cd6171`，對應目前 NSIS 更新清單及 installer-verification.json。版本仍為 0.8.8.0，主程式 EXE 與本文件上方 SHA256 相同；歷史離線 ZIP 未重製。
