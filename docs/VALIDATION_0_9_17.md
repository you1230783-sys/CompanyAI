# 0.9.17 驗證紀錄

尚未完成、尚未發布。M11 依序處理資源鎖、兩個專案任務、排程名額。公司真模型驗收由使用者進行。

## 資源鎖基礎

- scripts/Enter-DevShell.ps1 固定 v142 14.29.30133 x64；cargo test --frozen resource_locks:: -- --test-threads=1：4項通過，見本機 .build/resource-locks-0917-r1.log。
- 實際雙執行緒 FIFO、另一種資源可進行、取消、逾時、排序群組、重入及 panic 票券／guard 釋放。
- 此階段只證明鎖模組，尚未宣稱 Office／UI／排程整合或安裝包通過。

## 公司端追加回饋

- read_excel_range 成功沒有接入閱讀筆記來源登記，既有錯誤訊息誤導模型反覆猜測 operation_id；待修。
- read_work_log 出現工具原始結果校驗失敗，正在重現封存前後 JSON 數值序列化差異；原因未驗證前不宣稱已定位。
