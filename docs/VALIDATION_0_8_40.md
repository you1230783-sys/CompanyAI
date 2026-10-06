# 0.8.40：Excel 欄位規劃與使用者圖表設定

## 行為

AI 先核對每個檔案的表頭及少量樣本，解釋使用者用詞，再用 `plan_excel_analysis` 分開綁定時間篩選、X、Y。本機核對實際表頭、來源版本及 Excel NumberFormat，攔截時間小數充當量測Y；真正分析時間仍可明確指定 `y_kind=time`。語意對應仍由模型判斷，不保證任意別名必定正確。

`export_planned_excel` 只讀必要欄。B/D 不會變成 A/B；時間篩選在本機，最多掃250000列／120秒、符合最多10000列／9欄。HH:MM為時:分，區間起點包含、終點不含；一天內時間可跨午夜，終點可24:00。日期序號不默默去日期，空白／錯誤時間與超額不默默忽略或抽樣。回傳掃描完整性；模型須處理未掃描尾段。

CSV第一筆 `__display` 可為 version=2 物件，保存表頭、格式、來源及規劃；舊CSV文字陣列仍相容。`chart_dataset` 重查版本與鎖定欄位；DPAPI續接也保存規劃。規劃過的來源不改走舊匯出／直接圖表入口。

完成訊息的圖表提供「編輯圖表」、雙擊、恢復原樣及儲存此圖片。可改標題、實體X/Y軸名與範圍、各系列圖例名稱／顏色、圖例位置，以及最多10條水平／垂直參考線。類別軸使用完整標籤或 #資料序號（1起算），重複標籤不猜位置。設定保存於本機加密歷史，不送模型、不改原始點陣；進行中的預覽需待成果訊息出現後編輯。

標題固定置頂置中，圖例可選右側／右下／下方／標題下方／隱藏，本版沒有自由拖曳排版。圖型為折線、直條、散佈、階梯線、面積、水平長條；沒有額外統計的直方圖／箱形圖。

手動PNG為1600×1000白底，包含自訂設定及明確座標範圍；聊天室臨時滑動縮放不影響匯出。保存於專案 `_AI_Output/<當前時間資料夾>/自訂圖表.png`，不覆寫舊檔，原生檢查PNG及讀回一致後才報成功。來源圖上不印版本hash，仍保留可讀檔名、工作表、欄位、原始列範圍及資料處理說明。版本仍供內部來源追蹤。

## 驗證紀錄

2026-10-06 完成最終版本的完整建置，結束碼為 0：

```powershell
.\scripts\Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build'
```

- 固定 Rust 1.98.1、MSVC v142 14.29.30133、Windows SDK 10.0.19041.0；實際 x64 編譯器探針為 `_MSC_FULL_VER=192930159`。使用空 Cargo 快取、既有 Cargo.lock／vendor／.cargo/config.toml，以 `--frozen` 建置；格式、Clippy、246 項 Rust 單元測試均通過。
- 真實 Excel COM 建立三份各 1440 列的 XLSX，其中一份交換時間與量測值的 B／D 欄。每份篩選五個時段，產出共 15 張圖，逐一核對全部 900 個點的時間及數值。錯誤時間 Y 被拒絕，未選欄內容不進入規劃／匯出結果，來源檔案位元組不變。另驗證規劃隨 CSV 及 checkpoint 恢復、來源／表頭／角色不符時拒絕作圖。
- WebView2 與原生控制器測試通過；14 個原生介面案例包含圖表設定的 DPAPI 保存、舊訊息識別拒絕、原始數值保持與恢復原樣。六種圖型、自訂軸範圍、參考線、顏色及來源版本碼隱藏亦已驗證。
- 由最終 EXE 實際產生 1600×1000 自訂 PNG，透過原生回呼保存至 `_AI_Output` 並讀回比對。目視檢查匯出圖片的置中標題、右下圖例、上下限及垂直參考線、色彩與來源說明；此項不代表所有介面或任意資料版面皆經人工檢查。
- 22 個原生任務 HTTP 案例、5 個既有圖片請求案例、30 份約 10 MiB LOG、跨批 checkpoint／中斷恢復、PDF 與記憶快取回歸通過。圖片案例使用本機模擬服務，不是公司模型辨識驗收。
- Office 十種格式讀取／修訂／保存／重開、三種新建文件、XLS／XLSX 各一萬點圖表及 CSV、Word／Excel／PowerPoint 圖片嵌入回歸通過。localhost SMB 的 UNC 與臨時映射 R 槽也完成 TXT、DPAPI、Word／Excel 保存及來源保護驗證。

建置紀錄位於本機 `.build/build-0.8.40-final.log`；可交付的機器驗證摘要為 [EXE](../offline/exe-verification.json)、[原生介面](../offline/composer-verification.json)、[Office](../offline/office-verification.json)、[網路路徑](../offline/network-verification.json)。

本次交付 `dist/LM_AI.exe`（檔案版本 `0.8.40.0`）及相符的已簽署 EXE 更新清單，未重製 NSIS 或歷史離線 ZIP。

| 項目 | 結果 |
| --- | --- |
| EXE 長度 | 14,529,536 bytes |
| SHA-256 | `434db9da29c1766177b6f5fd397f7394566f7e800262df8dadb8520399204ca6` |

圖片大小與自動壓縮依使用者指示延至下一版；本版維持單張原圖 1 MiB。既有圖片請求的完整 JSON 形狀見 [IMAGE_REQUEST_EXAMPLE.json](IMAGE_REQUEST_EXAMPLE.json)，其中 ID、Base64 為示意占位值。

測試使用獨立本機資料；公司加密Excel、F槽、真實GLM語意選欄及公司圖片代理尚待使用者實測。原始四欄問題以合成資料重現，不宣稱已取得使用者原檔。
