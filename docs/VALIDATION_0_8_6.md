# LM_AI 0.8.6：EXE 與 VNC 驗證

## 本次交付

- `dist/LM_AI.exe`、`dist/update-manifest-exe.json`：0.8.6 主程式與配對的 RSA／SHA256 更新清單。
- 原始碼、Cargo.lock、前端、開發驗證工具與文件。
- `offline/exe-verification.json`、`offline/vnc-verification.json`、`offline/environment.txt`：本版驗證與工具鏈記錄。
- 依使用者指示，本輪不製作 NSIS 或離線 ZIP。既有 Setup／NSIS 清單仍為 0.8.5，舊 ZIP 不代表 0.8.6。

## 功能

VNC 功能預設關閉；設定勾選啟用後才顯示側欄。原生層同樣檢查開關，不需要 AI 登入，不會將機台資料或密碼送往網站。設定檔固定沿用 EXE 旁 `machines.json`／`user_config.json` 的原 Python 格式；支援機台管理、手動上移／下移，編輯保留原位置，不自動排序。Viewer 路徑可搜尋／指定，三個連線選項為全螢幕、唯讀、自動縮放。

Outlook 日期範圍預設為目前資料夾及子資料夾，移除所有信箱選項。一般／背景模式與快捷鍵提示同列，提示及送出按鈕說明隨 Enter 設定、自訂選字快捷鍵與啟用狀態更新。

## 驗證方式

執行 `scripts/Build.ps1 -EmptyCargoCache -TestVnc`：先以 `vcvars64.bat -vcvars_ver=14.2` 載入 v142 x64，編譯器探針核對 `_MSC_VER`／`_M_X64`，再執行 fmt、Clippy、Rust 測試、空 Cargo 快取的 `cargo build --workspace --release --frozen`、WebView2 自檢、真實 UltraVNC 測試與 EXE 更新清單驗證。依賴仍由 Cargo.lock、vendor、.cargo/config.toml 提供，沒有新增套件。

真實 Viewer 使用本機已安裝的 UltraVNC 1.8.2.4；整合工具以正式 `vnc::connect()` 讀取隔離的 Python 格式設定並啟動 Viewer。僅連 `127.0.0.1`，測試端驗證 RFB 3.8、傳入密碼的 challenge-response、shared flag 及 framebuffer request，傳送合成色塊。兩種案例為預設選項，以及全螢幕／唯讀／停用縮放。測試成功或失敗均只關閉本次啟動的 PID。

2026-09-22 本機驗證通過：MSVC 14.29.30133、實際 `_MSC_FULL_VER=192930159` x64；fmt、Clippy、81 項 Rust 測試、空快取 frozen release、WebView2 自檢、兩次真實 Viewer 整合測試與 EXE 清單簽章／SHA256 驗證均成功。

成品版本 `0.8.6.0`，大小 `4,482,560` bytes，SHA256：`a1803e19ee5ae790235de2fbccc7f5f8702b11d7bf53f266e7f855c2f0ffa5a6`。EXE 與 VNC 驗證記錄、更新清單均對應相同成品。

## 驗收界線

這是「真實 Viewer 對本機模擬 RFB 端」測試，沒有使用真實機台密碼或連線公司機台。畫面更新要求與密碼驗證有協定記錄；因 Computer Use 未獲准存取 UltraVNC 視窗，沒有截圖驗收，也未驗證全螢幕尺寸、縮放外觀或唯讀輸入攔截。

公司防毒、Outlook 實機、實際機台的遠端權限、Viewer 版本差異、重新連線行為，以及未來 0.8.6 NSIS 安裝／升級仍需各自驗收。Git 上傳不代表已部署內網網站；EXE 更新仍須由使用者退出舊程式後手動替換。
