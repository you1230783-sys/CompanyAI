# 0.8.52：Outlook相容性與五項分析審閱改善

## 行為與界線

1. 完成摘要計算實際工具的檔案／處理範圍、失敗、部分與筆數核對；條件掃描完成不等於全文或結論已驗證。
2. LOG事件透過record_log_events引用成功操作的完整原文行；原生取得來源、revision、行號、日期與時間。開始／結束／取消只有單一起點及明確先後才配對；缺漏、多起點、同時刻、沿用時間、同檔不同版本、日期不明、時間倒退不猜測。分類及group是AI判斷，順序不是因果。每次1–50筆、任務上限500筆或1.5MB。
3. 副本保存建立時基準及各次已驗證成果的快照，沿用DPAPI歷史與checkpoint，不能用新原件補造舊比較。左右唯讀、紅刪綠增、共同行捲動、搜尋、變更篩選、跳轉、字級與放大。Office是文字區塊及格式／結構內容比較，不是實際頁面排版。超大區段使用有界粗略對齊並提示，沒有刪去內容；每頁呈現300列，可續載。舊無基準明示不可還原，資料匯出／圖片不保證有比較資料。
4. 重用專案分析方法只列同專案未刪除的方法筆記，可修改步驟及本次資料／條件再填入提問草稿，不直接執行舊程式。新資料仍需核對來源、格式、欄位及樣本。
5. 文件搜尋逐檔回報範圍、版本、失敗與未列出的命中；預覽來源、行號／Office區塊、最多2000字前後文。每次命中最多60處、面板200處；所有截斷均明示，版本不同提示核對。預覽僅表示當時快照，非目前檔案即時內容。

Outlook Table的ConversationID改為選用；不支援時留空，必要欄位仍須成功，GetArray按實際欄數解析。新增205封跨三批支援／拒絕該欄位、必要欄位失敗案例。沒有額外讀取正文、附件或地址簿；隱藏副本鍵仍是完整主旨與寄送時間秒數。

沒有加入通用PowerShell／CMD執行器、實際Office頁面預覽、逐項接受／拒絕，沒有改動Python runtime／worker及歷史Rust離線ZIP。

## 驗證狀態

2026-10-12 13:59（Asia/Taipei）完整發行建置與安裝驗證通過。

- Rust 1.98.1、MSVC v142 14.29.30133、SDK 10.0.19041.0；實際 `_MSC_FULL_VER=192930159 x64`。Cargo.lock、vendor與專案 `.cargo/config.toml` 搭配空Cargo快取及 `--frozen`。
- fmt、Clippy（警告視為錯誤）、314項單元測試及release編譯通過。正式EXE與正式NSIS安裝後的WebView2／Python自檢通過。
- 新功能20項Edge互動與亮色／深色／窄視窗三種版面檢查通過；同組互動案例也在WebView2自檢執行。原生AppContainer／broker實測搜尋失敗與截斷、Unicode原文、兩次成果版本、原件改變後固定基準與LOG原文配對。
- 單元測試涵蓋Office格式索引、舊資料缺基準、checkpoint／DPAPI歷史還原與錯誤成果路徑拒絕、部分／失敗範圍、LOG配對歧義及專案方法隔離。
- Outlook COM fixture的205封跨三批支援／拒絕ConversationID與必要欄位失敗通過；正式EXE輔助程序的無效要求／EOF退出通過，不接觸信箱。
- 26個原生代理、9個恢復、13個圖片案例及兩組68輪長任務通過；PDF、加密記憶／中斷續接與30份約10MiB LOG分頁通過。這些是本機合成案例。
- 真實Python副本功能驗收、兩程序並行、超過77MiB的Big5 LOG、檔案／網路／程序隔離、取消與120秒逾時通過。
- Word／Excel／PowerPoint十種既有格式、新建三種格式、15個Excel規劃圖表、大型xls／xlsx的10000點圖表及六份文件移除外部PNG後重開通過。
- localhost SMB的UNC／映射磁碟通過。完整NSIS安裝、更新、PID交接重啟、失敗回復、Python損壞修復、使用者資料保留及解除安裝通過；測試安裝已清除。

報告：[本版新增功能](../offline/review-verification.json)、[EXE](../offline/exe-verification.json)、[Python](../offline/python-verification.json)、[Office](../offline/office-verification.json)、[SMB](../offline/network-verification.json)、[NSIS](../offline/installer-verification.json)、[環境](../offline/environment.txt)。本機完整日誌：`.build/release-0.8.52-final.log`。

```powershell
./scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller
```

本輪首次完整流程在安裝後的串流閱讀位置自檢失敗，未發布該次產物。檢查發現新增方法重用測試未等待250ms草稿防抖，結束後仍可能把測試草稿送回原生狀態；已修正為攔截並核對實際draft命令、清除殘留計時器後才恢復測試環境，保留閱讀位置斷言。修正後獨立Edge的20項互動與三種版面檢查通過，完整發行流程重跑後全部通過。


## 交付產物

| 檔案 | 版本 | Bytes | SHA256 |
| --- | --- | ---: | --- |
| `dist/LM_AI.exe` | 0.8.52.0 | 17,821,184 | `33f2dc6438e63e94f42f7972c761c6d2084718f271c296b7d5865071d115d8f9` |
| `dist/LM_AI_Setup.exe` | 0.8.52.0 | 28,711,353 | `1652c5903692094a421add21f3ace1eac578ef771732f0d0fa402f910a8a7f56` |

兩份更新清單完全相同，簽章已由原生更新器驗證，均指向上述NSIS。Python worker／runtime、icon.png與歷史Rust離線ZIP未改動。

## 公司實測界線

本機COM fixture不代表公司Outlook資料提供者已實測；本機合成模型測試不代表真實模型會選到正確事件／原因，也不能代替公司加密與網路環境驗收。Git發布不代表公司內網網站已部署。
