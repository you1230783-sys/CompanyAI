# 0.8.52：Outlook相容性與五項分析審閱改善

## 行為與界線

1. 完成摘要計算實際工具的檔案／處理範圍、失敗、部分與筆數核對；條件掃描完成不等於全文或結論已驗證。
2. LOG事件透過record_log_events引用成功操作的完整原文行；原生取得來源、revision、行號、日期與時間。開始／結束／取消只有單一起點及明確先後才配對；缺漏、多起點、同時刻、沿用時間、同檔不同版本、日期不明、時間倒退不猜測。分類及group是AI判斷，順序不是因果。每次1–50筆、任務上限500筆或1.5MB。
3. 副本保存建立時基準及各次已驗證成果的快照，沿用DPAPI歷史與checkpoint，不能用新原件補造舊比較。左右唯讀、紅刪綠增、共同行捲動、搜尋、變更篩選、跳轉、字級與放大。Office是文字區塊及格式／結構內容比較，不是實際頁面排版。超大區段使用有界粗略對齊並提示，沒有刪去內容；每頁呈現300列，可續載。舊無基準明示不可還原，資料匯出／圖片不保證有比較資料。
4. 重用專案分析方法只列同專案未刪除的方法筆記，可修改步驟及本次資料／條件再填入提問草稿，不直接執行舊程式。新資料仍需核對來源、格式、欄位及樣本。
5. 文件搜尋逐檔回報範圍、版本、失敗與未列出的命中；預覽來源、行號／Office區塊、最多2000字前後文。每次命中最多60處、面板200處；所有截斷均明示，版本不同提示核對。預覽僅表示當時快照，非目前檔案即時內容。

Outlook Table的ConversationID改為選用；不支援時留空，必要欄位仍須成功，GetArray按實際欄數解析。新增205封跨三批支援／拒絕該欄位、必要欄位失敗案例。沒有額外讀取正文、附件或地址簿；隱藏副本鍵仍是完整主旨與寄送時間秒數。

### 同版修正：Outlook Table 原生陣列方向

使用者以唯讀小程式在公司Outlook16.0.0.17932／64位元實測，確認原生SAFEARRAY的D1為郵件列、D2為欄位。GetArray(1)回傳1×5及1×9，GetArray(100)回傳100×5及100×9；Columns.Count與必要欄位一致，ConversationID確實不支援。原解析器把維度判反，因此將100列誤認為100欄，觸發「Table批次大小不符」。先前COM fixture也採相反排列，造成測試未能找出此錯誤。

修正維度查詢及GetElement索引為「列、欄」，維持每批100列，並在尺寸異常時回報兩維邊界、實際列欄數與預期欄數。回歸測試使用公司實測方向，涵蓋5／9／10欄、1至100列、正方形批次、非零下界、空批次、異常型別／維度／尺寸，以及205封跨批次的逐欄內容核對。未增加逐封郵件讀取或依郵件內容猜測排列。

診斷工具：`scripts/Test-OutlookTable.ps1`，使用Windows PowerShell5.1與內建.NET，直接檢查IDispatch回傳的原生SAFEARRAY，只輸出欄位狀態及尺寸。`-SelfTest`以合成COM物件核對兩種排列，不接觸Outlook。公司回報確認的是陣列方向；修正版完整郵件讀取仍需公司實測。

沒有加入通用PowerShell／CMD執行器、實際Office頁面預覽、逐項接受／拒絕，沒有改動Python runtime／worker及歷史Rust離線ZIP。

## 本次修正版驗證與交付狀態

2026-10-12T15:23:40+08:00：維持0.8.52，已完成v142編譯與NSIS封裝；依使用者最新指示停止剩餘驗證並發布。**本次不是完整整合與安裝驗證通過的版本紀錄。**

- Rust1.98.1、MSVC v14214.29.30133、SDK10.0.19041.0；實際`_MSC_FULL_VER=192930159 x64`。Cargo.lock、vendor及專案.cargo/config.toml搭配空Cargo快取／--frozen。
- fmt、Clippy、319項單元測試與release編譯通過；其中Outlook專項30項通過，含本次新增五項陣列測試。
- 新EXE的Outlook輔助程序協定／EOF、WebView2及Python啟動自檢通過。實際AppContainer副本、來源搜尋／比較／LOG事件、30份約10MiB LOG及原生案例0–21（含兩組68輪）已通過。
- 使用者要求停止後，不再執行剩餘整合案例、Office／SMB及完整NSIS安裝／更新／回復／解除安裝驗證。沒有把尚未完成的項目列為通過。
- 使用這次release執行檔製作NSIS，產生兩份相同的簽署更新清單；正式EXE的更新器已核對安裝包與簽章。

本次報告：[Outlook修正](../offline/outlook-table-verification.json)、[EXE](../offline/exe-verification.json)、[NSIS封裝](../offline/installer-verification.json)。本機日誌：`.build/release-0.8.52-outlook-layout.log`（依指示中止）、`.build/package-0.8.52-outlook-layout.log`（封裝成功）。

## 交付產物

| 檔案 | 版本 | Bytes | SHA256 |
| --- | --- | ---: | --- |
| `dist/LM_AI.exe` | 0.8.52.0 | 17,796,096 | `d69316d650100ed0164159fd7efc59d3f464d1f2076f73182becd1a635e7a08e` |
| `dist/LM_AI_Setup.exe` | 0.8.52.0 | 28,689,782 | `06cec6e7aeb299f8e6efd1e0f8891a77fdeaf4060f3c46ecb29fd82116ff4a53` |

此修正版仍為0.8.52，測試時須重新取得這次NSIS，不以畫面版本號判斷是否已換成修正版。Python worker／runtime、icon.png與歷史Rust離線ZIP未改動。

## 先前完整驗證紀錄

2026-10-12 13:59的314項單元測試、完整Office／SMB／NSIS與五項審閱功能驗證，對應初次0.8.52的舊產物，不能套用為本次修正版完整驗收。原始紀錄見[初次發行提交](https://github.com/you1230783-sys/CompanyAI/blob/31b0f3bc298e877a1aad23a9a9cf94eaa640fbac/docs/VALIDATION_0_8_52.md)；`offline/review-verification.json`及未更新的Office／Python／SMB等報告保留當時時間與範圍。

## 公司實測界線

公司實測已確認原生陣列方向與ConversationID不支援；修正版的完整信件讀取及速度仍待公司驗收。本機合成模型測試不能代替公司加密、網路及真實模型品質測試。Git發布不代表公司內網網站已部署。
