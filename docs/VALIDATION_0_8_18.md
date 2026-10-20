# 0.8.18 驗證紀錄

2026-09-29 執行 `scripts/Build.ps1 -EmptyCargoCache -TestOffice` 成功。此文件描述本機測試，不代表公司加密環境驗收。

- MSVC v142：VCToolsVersion 14.29.30133，實際 `_MSC_FULL_VER=192930159`、x64；Rust 1.98.1。
- Cargo.lock、vendor、.cargo/config.toml 齊備。全新空 Cargo 快取下 `--frozen`、fmt、Clippy `-D warnings`、118 項測試與 release 建置通過。
- 真正 WebView2 自檢通過：code、Markdown 連結與獨立純文字成果路徑皆轉為可讀連結，點擊送出正確原生定位命令。
- 另以本版原生函式實際開啟 Explorer，透過 Shell COM 確認 `修訂_2.docx` 已在正確目錄被選取；越界及非成果路徑被拒絕。僅關閉本次測試新建視窗。
- 以虛構資料擷取並檢視桌面前端成果連結截圖；另外 20 項既有 UI／VNC 對話框回歸檢查通過。沒有宣稱 Office 文件版面已逐頁截圖驗收。
- 真正 AppContainer EXE 檔案／網路隔離、文字副本、操作重播、原件保留、越界及硬連結拒絕通過；三組 HTTP skills/tool loop、JSON 修正與歷程測試通過。
- 目錄命名及同名檔案不覆寫；目錄更名／寫入阻擋；Office 換鎖期间 anchor 防更名及不可刪除通過。實際 FSCTL_SET_REPARSE_POINT 設 Junction 被 Windows 以 ERROR_DIR_NOT_EMPTY (145) 阻擋，確認不是無效測試參數造成拒絕。
- 本機 Office 16.0.20326.20158 實際產生 DOCX/XLSX/PPTX，經正式 broker 建立副本、修改、儲存、重新開啟、finish 檢查，原檔 bytes 不變。
- 驗證可讀檔名、修改後第二次另存 `_2`、相同版本重送不重複寫檔。Excel 數字保持數值、公式拒絕改動、`=1+1` 按字面文字保存。另由 Office 重開確認 Word／Excel 粗體與未修改的段落、公式、投影片文字框保留。
- EXE：6,012,416 bytes；SHA256 `df118010866f8728bf79ecb4739d07684c5153f4c82c057f43e74249d766929c`，與簽署 EXE 更新清單一致。

機器可讀結果：`offline/exe-verification.json`、`offline/office-verification.json`。本輪不重建 NSIS 或歷史離線 ZIP；安裝包及其清單保留 0.8.15。

尚需公司測試：加密 Office 原件解密、Office 另存／broker 複製觸發加密後能否讀回、實際模型工具選擇、複雜格式與第三方 Office 加入項、公司信任中心／權限政策。Office 呼叫中若出现原生等待視窗，取消要等該呼叫返回，不保證立即終止 Office。
