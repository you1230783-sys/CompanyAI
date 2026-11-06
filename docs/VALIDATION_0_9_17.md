# 0.9.17 驗證紀錄

尚未完成、尚未發布。M11 依序處理資源鎖、兩個專案任務、排程名額。公司真模型驗收由使用者進行。

## 資源鎖基礎

- scripts/Enter-DevShell.ps1 固定 v142 14.29.30133 x64；cargo test --frozen resource_locks:: -- --test-threads=1：4項通過，見本機 .build/resource-locks-0917-r1.log。
- 實際雙執行緒 FIFO、另一種資源可進行、取消、逾時、排序群組、重入及 panic 票券／guard 釋放。
- 此階段只證明鎖模組，尚未宣稱 Office／UI／排程整合或安裝包通過。

## 公司端追加回饋

- read_excel_range 成功沒有接入閱讀筆記來源登記，既有錯誤訊息誤導模型反覆猜測 operation_id。現已接入 Excel 結構化來源，記錄實際工作表／欄列／範圍；舊成功封存可補登記，失敗操作拒絕並列出可引用來源。
- read_work_log 出現工具原始結果校驗失敗，以合成科學數值確實重現（.build/archive-numbers-0917-r1.log），原因為 JSON 預設浮點解碼後重新編碼產生差異。啟用 float_roundtrip，新封存保存原始 JSON 字串並先核對原文雜湊；舊格式相容、真正竄改仍拒絕。

- v142 定向回歸：12項記憶／封存及2項Excel筆記引用通過（.build/memory-0917-r3.log、.build/excel-notes-0917-r3.log）。包含大結果分塊、原始小數／極小值／負零、舊格式、竄改拒絕、成功Excel封存補登記、不因錯誤ID部分保存筆記。尚待整合與公司資料實測。
