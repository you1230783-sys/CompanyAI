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

安裝包版本 `0.8.8.0`，大小 `217,573,495` bytes，SHA256：`f14e4b51bad516c2cb6fd8c1454bac0ce9f6eb624663c749188b325238006686`，與 `dist/update-manifest.json` 及 `offline/installer-verification.json` 一致。主程式仍為上方已驗收的相同 SHA256，未修改功能或進版。
