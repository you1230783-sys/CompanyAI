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

沿用使用者前輪 EXE 優先交付與 Git 推送要求，不製作 NSIS 或離線 ZIP；舊 NSIS 維持 0.8.5。公司端實際選檔／轉檔仍需使用者以新版驗收。本次未修改 VNC，不重做 Viewer 測試。
