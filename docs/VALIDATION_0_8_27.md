# 0.8.27 驗證紀錄

2026-10-01 執行 `scripts/Build.ps1 -EmptyCargoCache` 全部通過，exit code 0。

- MSVC v142 14.29.30133，實際探針 `_MSC_FULL_VER=192930159`、x64；Rust 1.98.1、Windows SDK 10.0.19041.0、靜態 CRT。
- Cargo.lock、vendor、.cargo/config.toml 齊備、無新增依賴；全新空 Cargo 快取完成 fmt、Clippy `-D warnings`、163 項單元測試及 `cargo build --workspace --release --frozen`。
- EXE 版本 0.8.27.0、9,542,144 bytes，SHA256 `465aabc89e82bc8223431eef8582e9af554dd9bcb00bb7af1789ee0ea2fbf0be`。EXE／簽署清單／機器紀錄一致；發行腳本以正式 EXE 內建公鑰驗證清單及成品。
- WebView2 DOM self-check、真正 AppContainer 檔案／網路隔離、工具整合與既有 PDF／筆記／快取回歸通過。

## 新增實測

以真實 loopback HTTP、DPAPI 與隔離程序驗證：

| 情境 | 結果 |
| --- | --- |
| 等待模型時到達期限 | 暫停後查同一 ID，4 次 POST／2 次 GET 完成任務。 |
| 完成回覆剛好超時 | 保留已驗證回覆，續接不再 GET 或重新生成；4 次 POST／0 次 GET。 |
| 連續三次壞格式 | 有限修復用盡後暫停，按繼續才重新請模型處理；7 次 POST。 |
| POST 503 且 GET 404 | 保存待查請求，手動續接只查回原請求；4 次 POST／4 次 GET。 |

四種情境都保留同一副本及未儲存文字，暫停前沒有輸出，續接後只產生一份正確成果。既有 60 次工具暫停／去重、24／26 輪長文件、取消、識別碼錯誤、空白回覆及無進展回歸通過；取消與識別碼錯誤仍停止。

機器紀錄：`offline/exe-verification.json`、`offline/environment.txt`。`offline/office-verification.json` 保留 0.8.26 的 Office 實測紀錄，不視為本版重測。

時間邊界使用 debug 專用短時鐘，另測正式時限常數為 7200 秒；沒有實際等待兩小時，不開放正式 EXE 設定時限。沒有更改 Office COM、沒有重跑 Office／Outlook／VNC 實機；公司模型、加密與內網時限仍待公司實測。不製作 NSIS 或離線 ZIP。
