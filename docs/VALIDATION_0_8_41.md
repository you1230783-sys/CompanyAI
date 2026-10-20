# 0.8.41：圖片試驗與閱讀位置

## 行為

圖片及原生快速摘要子請求直接引用父請求的遠端對話ID；新client_request_id、相同owner/project/run/conversation。網站使用本次client_snapshot，不自行拼接歷史。舊版本因422停下的任務請重新送出新任務；不改寫舊未知提交的ID或內容。

JPG/JPEG/PNG原檔最大5 MB（5,000,000 bytes），每次任務只允許一次不同辨識要求；同來源版本及焦點可重用已完成結果。超額由使用者自行縮小，不自動壓縮。維持8192×8192、1600萬像素限制。Base64後整份JSON上限10,000,000 bytes，仍遵守網站capabilities.limits.request_bytes公告；只放寬網站body parser而未更新公告，仍可能在桌面預檢被拒。

圖片僅送當次無工具子請求；下一輪及後續對話只有文字重點和來源。辨識提示要求依目的保留名稱、價格、單位、規格與對應關係；看不清及未涵蓋內容需明示。不是完整逐字轉錄保證，也沒有新增批次圖片功能。待查請求在本機DPAPI保存，避免斷線後重送；成功或網站明確回task_accepted=false後移除該圖片快照。桌面無法驗證網站資料庫是否保存Base64。

fast模型不公告圖片技能或analyze_image，也不能由原生入口或工具繞過。圖片按鈕提示「此模型不支援圖片傳入」，不自動改用其他模型。Office插圖與圖表輸出不需要模型看圖，仍可使用。

工具紀錄使用同一清單增量追加；讀到中間時保持位置，在底部才跟隨。120筆上限輪替時保留仍存在的可見列；已移出上限的舊列無法保持。進度筆記和已完成訊息的內部捲動也保留。不同對話／任務重設各自顯示。

## 驗證

2026-10-06 完成最終版本的完整建置，結束碼為 0：

```powershell
.\scripts\Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build'
```

- Rust 1.98.1、MSVC v142 14.29.30133、Windows SDK 10.0.19041.0；實際 x64 編譯器探針為 `_MSC_FULL_VER=192930159`。以空 Cargo 快取、既有 Cargo.lock／vendor／.cargo/config.toml 執行 `--frozen` 建置；格式、Clippy 及 246 項 Rust 單元測試通過。
- 9 個真實本機 HTTP 圖片案例通過：PNG／JPEG 原始 bytes 一致、結果重用、未知提交只查原 ID、專案外來源拒絕、5,000,001 bytes 拒絕、5,000,000 bytes 接受、快速模型零圖片請求、明確 422 拒絕後完成清理、第二張圖片被單次任務額度阻擋。每個父子請求核對 owner／project／run／conversation，後续主請求不含圖片 data URL。
- 另以本次自行產生的成功、斷線續接及明確拒絕案例，檢查 28 份本機 DPAPI 測試檔；3 個已完成圖片狀態均已清除 pending，未發現保留的圖片 data URL。此項未讀取使用者資料，亦不代表網站資料庫驗證。
- WebView2 DOM 與 14 個原生介面案例通過；包含快速模型圖片入口阻擋、工具清單增量追加、中途閱讀位置保持、底部跟隨、120 筆輪替的可見列保持、進度筆記不因普通工具更新重建。
- 22 個原生任務 HTTP 案例通過，包括快速摘要子請求的相同遠端對話約束。長任務 68 次請求、中斷續接、未知提交去重、PDF／加密記憶與 30 份約 10 MiB LOG 回歸通過。
- 真實 Excel COM 三份不同欄序檔案 × 五個時段，共 15 張圖，核對全部 900 個點。Office 十種格式、三種新建文件、XLS／XLSX 一萬點圖表及 CSV、Word／Excel／PowerPoint 圖片嵌入與保存重開均通過。
- localhost SMB 的 UNC 與臨時映射 R 槽完成 TXT、DPAPI、Word／Excel 保存重開、路徑越界拒絕與原始檔保護驗證；未以此宣稱公司 F 槽已驗收。

本機完整紀錄：`.build/build-0.8.41-release.log`；機器驗證摘要：[EXE](../offline/exe-verification.json)、[原生介面](../offline/composer-verification.json)、[Office](../offline/office-verification.json)、[網路路徑](../offline/network-verification.json)。

交付 `dist/LM_AI.exe`（檔案版本 `0.8.41.0`）及相符的已簽署 EXE 更新清單；未重製 NSIS 或離線 ZIP。

| 項目 | 結果 |
| --- | --- |
| EXE 長度 | 14,562,816 bytes |
| SHA-256 | `3faafb37352025b906098d135b82ee4648dd03c4c37739a808b06b942e7a7373` |

請求格式見 [IMAGE_REQUEST_EXAMPLE.json](IMAGE_REQUEST_EXAMPLE.json)，ID 與 Base64 為示意值。

公司實際模型辨識品質、代理服務與加密／網路環境仍待使用者實測。本機HTTP案例不代表公司服務已部署。
