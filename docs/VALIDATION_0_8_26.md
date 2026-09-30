# 0.8.26 驗證紀錄

2026-09-30 執行 `scripts/Build.ps1 -EmptyCargoCache -TestOffice` 全部通過（exit code 0）。

EXE 版本 **0.8.26.0**，大小 **9,481,216 bytes**；SHA256：

```text
87d66e2eef7a805a4e4b0da56d968c137be1e706b1829599ea06c70795f9ffaa
```

EXE、簽署更新清單及機器驗證紀錄一致。發行腳本使用 EXE 內建公鑰與正式更新驗證流程核對簽章及檔案。

## 驗證範圍

- Windows 11 x64、Rust 1.98.1、MSVC v142 14.29.30133；以實際編譯器探針核對 `_MSC_FULL_VER=192930159`、x64，Windows SDK 10.0.19041.0、靜態 CRT。
- Cargo.lock、vendor、.cargo/config.toml 齊備，沒有新增相依套件。使用空 Cargo 快取執行 fmt、Clippy `-D warnings`、162 項單元測試及 `cargo build --workspace --release --frozen`。
- WebView2 DOM self-check 包含「繼續」與「重新再試一次」的區分、執行中禁用及重複點擊防護；真正 AppContainer 檔案／網路隔離與工具整合。
- 真實 loopback HTTP 執行 60 次工具後暫停，確認暫存為 DPAPI 密文。重新建立執行器後恢復同一工作副本及 58 次未儲存編輯，重用先前儲存操作 ID 不多發布一份，最後正常交付。錯誤專案 ID 拒絕續接且不消耗暫停點；續接上下文不重送整段工具歷史。
- 第 20 輪 POST 503、首次查詢 GET 404、之後查回完成的情境正常完成 24 輪；沒有重送 POST。持續無法查回的情境保留最初提交與最後查詢錯誤。
- 八項新舊文字工具協定往返、長文件 24／26 輪、重播去重、取消、錯誤身分、空白回覆、無進展上限回歸。
- `.lmai` 過濾、加密筆記 CRUD／復原、文件摘要、PDF 解析／伺服器 multipart／跨任務快取與來源變更失效回歸。

## 原生 Office 驗證

透過本機 Word、Excel、PowerPoint（Office 16.0.20326.20158）的固定 COM 呼叫建立測試資料，經正式 Broker 修改／儲存，再使用 Office 重新開啟核對，以下項目全部通過。

- DOC／DOCX／DOCM、XLS／XLSX／XLSM／XLSB、PPT／PPTX／PPTM：讀取、修訂、儲存與重開；原件 bytes 不變、未修改段落／文字框及粗體保留、可讀檔名與重名序號、重複儲存去重。
- 新建 DOCX：段落、標題／正文樣式、條列、中文及拉丁字型、顏色、行距、插入位置與簡單表格。
- 新建 XLSX：工作表、矩形資料範圍、標題格式、底色、框線、欄寬、換行與數字格式；既有公式受保護，以 `=` 開頭的文字維持字面值。
- 新建 PPTX：標題頁、標題＋內文、雙欄版面；文字、字型、字級、顏色、條列及段落間距。
- 格式快照共用樣式表，浮點數正規化，避免 Office 數值經 JSON 往返後出現微小差異而誤判來源變更。

機器紀錄：`offline/exe-verification.json`、`offline/office-verification.json`、`offline/environment.txt`。

驗證期間曾有一次 Word COM 啟動回報 `0x80080005`（伺服器執行失敗），該輪建置以失敗停止，沒有略過 Office 檢查。檢查未發現殘留 Office 程序後，重新執行完整驗證並全部通過。未將這次偶發啟動失敗宣稱為已找出根因或永久修復。

公司加密系統、內網 GLM／Gemma、正式回報的 404 原因與視覺版面尚未實測。HTTP 情境使用確定性回覆，不代表模型成功率已驗證。原生 COM 檢查不代替畫面與公司驗收；未重跑 Outlook／VNC 實機操作。

本版只交付 EXE、簽署清單、原始碼與文件，不製作 NSIS 或重製歷史離線 ZIP。Git 推送不代表公司內網已部署。
