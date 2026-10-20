# 0.8.28 驗證紀錄

2026-10-01 執行 `scripts/Build.ps1 -EmptyCargoCache -TestOffice` 全部通過，exit code 0。

- MSVC v142 14.29.30133，實際探針 `_MSC_FULL_VER=192930159` x64；Rust 1.98.1、Windows SDK 10.0.19041.0，靜態 CRT。
- Cargo.lock、vendor、.cargo/config.toml 齊備，無新增 Rust 依賴；全新空 Cargo 快取完成 fmt、Clippy `-D warnings`、165 項單元測試及 `cargo build --workspace --release --frozen`。
- EXE 0.8.28.0，11,030,528 bytes，SHA256 `ac6830495d644d508eee9b77408d6570c4168cf9d9883ca46941eb083f5e8e43`。EXE／簽署清單／機器驗證紀錄一致；發行腳本使用正式 EXE 內建公鑰驗證清單及成品。

## 新增實測

| 範圍 | 結果 |
| --- | --- |
| 側欄／Markdown | 獨立收合與偏好保存、舊設定預設展開；WebView2 測得專案說明表格與安全過濾。 |
| ECharts | 真正 WebView2 canvas、資料表、放大操作、惡意標籤當純文字；圖表數值／長度限制及去重。 |
| 技能／搜尋 | 載入內建技能、未知 id 拒絕；搜尋命中帶來源版本，`.lmai`／越界來源拒絕並標示不完整。 |
| 快速模型 | 真實 loopback HTTP 將 9,000 字切三段，只向 fast 送兩則訊息與 skills:false，不帶父歷史／工具定義；再次摘要使用 DPAPI 快取。 |
| 委派續接 | 第二段停留 running，短測試時限暫停後手動續接查原 ID；前三段總共只有三次 fast POST，已完成第一段不重做。 |
| 原生 Office | Office 16.0.20326.20158 驗證十種既有格式讀／改／另存／重開、原件不變；DOCX／XLSX／PPTX 新建以批次套用結構格式，失敗批次不提交版本；Excel 範圍直接取數值作圖。 |

既有 AppContainer 檔案／網路限制、長文件 24／26 輪、60 工具暫停、原請求查回、取消、識別碼錯誤、PDF／筆記與快取回歸均通過。首次測試因偏好欄位計數及技能文案的斷言尚未更新而失敗，修正後完整重跑通過；沒有略過檢查。

ECharts 6.0.0 來源為 Apache 官方 tag 的 `dist/echarts.min.js`；SHA256 `baa8dfe7e1d9336b98e8986ba7e20ea15e7cdbea1ef42a59d59478632fa45a1d`，LICENSE／NOTICE 同時保存於 ui/vendor。

機器紀錄：`offline/exe-verification.json`、`offline/environment.txt`、`offline/office-verification.json`。未實測公司模型、公司加密、內網效能；摘要準確度與文件外觀仍需使用者測試。圖表有 DOM/canvas 驗證，但未做外觀截圖審核。時間邊界使用 debug 短時鐘，沒有實際等待兩小時。本版不製作 NSIS 或離線 ZIP。
