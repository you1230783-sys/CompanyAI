# 0.8.13 EXE 驗證紀錄

驗證日期：2026-09-23。本輪依使用者要求先交付 EXE，待公司實測成功後再製作 NSIS。

## 功能範圍

- VNC 維持預設隱藏，勾選既有設定後才顯示。網站帳號、密碼與連線設定以目前 Windows 使用者的 DPAPI 加密保存；密碼不回傳前端。
- 只有按下更新才依序取得首頁 Cookie、登入、讀取機台 API 並嘗試登出；每次請求前等待 300ms，沒有定時更新或自動重試登入。
- 詳細設定提供根目錄、首頁、登入、登出及十個機台 API 相對網址，預設前兩個為 2F、4F。
- 下載結果先暫存在記憶體，按 eq_type 分類列出 machine_name，沒有 IP 的機台仍顯示。使用者勾選分類或個別機台後才匯入。
- 新機台 VNC 密碼預設 1234；更新既有機台保留自訂密碼、分類與順序。未勾選資料及手動建立的機台不會被同步刪除。
- 機台設定可多選機台上下移動／刪除，也可整個分類排序／刪除。空 IP 可以保存，連線時會提示補上 IP。
- 納入前次保留的更新提示用詞，明確說明重新啟動的是 LM_AI 應用程式。

完整設定與合併規則見 [VNC 快速連線](VNC_QUICK_CONNECT.md)。

## 已執行的驗證

執行 `scripts/Build.ps1 -EmptyCargoCache`，未指定 `-IncludeInstaller`。

| 項目 | 結果 |
| --- | --- |
| MSVC v142 x64 實際編譯器探針 | 通過；14.29.30133，`_MSC_FULL_VER=192930159` |
| Rust／SDK | Rust 1.98.1；Windows SDK 10.0.19041.0 |
| Cargo.lock、vendor、.cargo/config.toml 與全新空 Cargo 快取 | `cargo build --release --frozen` 通過 |
| rustfmt／Clippy | 通過，Clippy 警告視為錯誤 |
| Rust workspace 測試 | 101 項通過 |
| WebView2 DOM 自我檢查 | 通過，包含新增同步與多選操作 |
| JavaScript 語法 | vnc.js、vnc-sync.js、self-test.js 通過 Node 語法檢查 |
| EXE 更新清單 | 版本、長度、SHA256 與簽章驗證通過 |

新增測試使用 loopback HTTP 伺服器與虛構帳密，實際經過 WinHTTP，驗證 PHPSESSID 逐步更換、登入表單編碼、每次請求間隔、API 錯誤／取消時登出、登入失敗不重試、登出失敗提示、重複機台衝突、DPAPI 保存與網址界線、選擇性匯入、密碼保留、空 IP 及多選排序／刪除。

介面自檢涵蓋十個 API 欄位、記住帳號但不暴露密碼、無自動同步、更新／取消操作、預覽分類勾選與指定項目匯入、丟棄預覽、多選操作命令及停用 VNC 後清除介面資料。這是程式化 DOM 檢查，本輪沒有另做外觀截圖驗收。

機器可讀紀錄位於 [exe-verification.json](../offline/exe-verification.json)，編譯環境位於 [environment.txt](../offline/environment.txt)。

## 交付與未測範圍

- `dist/LM_AI.exe`：檔案版本 0.8.13.0，大小 5,122,560 bytes。
- SHA256：`f63ae20c067626acebf7f315e3790fc3c002a1560a8b632aa01db2361964aac6`。
- `dist/update-manifest-exe.json`：對應本次 EXE 的已簽署更新清單。
- 未連線公司網站、未使用真實帳密；公司登入、實際資料格式及匯入結果仍待使用者實測。
- 本輪未重新執行真實 VNC Viewer 連線，既有 0.8.6 紀錄不能視為本版實測。
- `dist/LM_AI_Setup.exe`、`dist/update-manifest.json`、`offline/installer-verification.json` 保留 0.8.12，未製作 0.8.13 NSIS，也未重新驗收安裝流程。
- 未重製歷史離線 ZIP；本次空快取驗證使用工作目錄內既有 vendor 與鎖定檔。
- 提交／推送至 Git 供下載測試，不代表公司內網下載路由已切換到 0.8.13。
