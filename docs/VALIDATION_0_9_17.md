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

## M11 整合與驗證過程（尚未宣稱發布完成）

- 每個對話各自持有 Running、進度、等待卡片、取消旗標；最多兩項，排程也占一個名額。同專案允許兩個對話，原來源副本操作依 canonical path 排隊。設定變更與安裝更新仍受工作中限制。
- Word／Excel／PowerPoint 在 COM 建立前取得 guard，關檔、恢復設定與 COM 引用釋放後才交還；Outlook 父程序與直接 COM 入口、CAD worker 派送同樣接入。多資源排序取得，不在使用者確認卡片等待期間占鎖。
- 專案記憶交易涵蓋完整讀改寫；方法筆記的查找與新增／修訂也跨短交易序列化。取消後仍保存已發生操作的審計。合成兩執行緒各12筆、共24筆完整保存已通過。
- 先發現外部 Word 檢查只看 CoCreateInstance 新實例，可能漏掉使用者文件；補查 ROT 已有文件。實際 Word A／B 排隊、Excel 在 A 持有 Word 時完成、兩份 Word 讀回及外部 Word BUSY 路徑已通過第二次 COM 測試。Python 與整份原生報告仍在後續重驗。
- 第一輪全量 Rust 發現新增 Excel 補登記先讀封存，遮住 Python 非閱讀來源的明確提示；調整為先拒絕 Python，再補登記 Excel。Windows 通知測試在受限桌面被拒，需在實際桌面重跑，不能算通過。
- 前端第一輪發現容量提示被快捷鍵／composer 重繪覆蓋；修正所有相關呈現。15項雙任務前端、19項分支、14項計畫、32項整合對話通過。新測試包括第三項拒絕、兩項停止身分、切換確認卡片不發送授權、排程不顯示遠端工作指令。
- Python 原生測試第一次誤向只處理一筆要求的已結束 worker 再發請求，回報 Windows 109；測試已改以新程序驗證取消隔離，保留失敗紀錄。未修改 runtime 或放寬隔離。
- Python 每個程序獨立 Job，1 GiB 程序上限。兩個任務各一個 Python 為2 GiB上限；若各自 run_batch 同時跑兩項，最壞最多4個 Python／4 GiB上限，另有主程式、Office與CAD記憶體。本機合成資料量測不代表公司大型資料的尖峰。

失敗與重試原始紀錄保留於本機 `.build/check-0917-r4.log`、`native-build-0917-r1.log`、`resources-0917-r1.err`、`resources-0917-r2.err`、`multi-task-ui-r1.log`～`r4.log`、`rust-0917-r1.log`；最終通過證據另列，不覆寫失敗紀錄。

## 最終整合追加發現

- 原生 UI 的舊停止斷言與跨對話 Finished 測試仍查目前畫面，已改驗卡片綁定的 conversation/run；過期完成不能移除其他任務。
- 真實雙 runner 自檢發現保存 history.dpapi 共享衝突。最小合成回歸確認：pin 持有所有祖先目錄的 GENERIC_READ，阻擋兄弟檔案原子替換。改成逐層檢查完成後只保留末層完整 handle；Windows 仍拒絕末層及包含其 handle 的祖先改名，非空祖先不能變成 Junction。短區間 mutex 協調 pin 建立過程與私有檔案原子替換，工具／模型／確認期間不持有。
- 曾嘗試 FILE_READ_ATTRIBUTES，因無法阻止改名被測試否決；MoveFileExW／ReplaceFileW 也未解決衝突，均已撤回。最終不放寬末層 WRITE／DELETE 分享。新增回歸同時驗證 64 次並行 pin／歷史替換及目錄改名拒絕。
- 相關失敗保留 .build/pin-sibling-0917-r1.log～r5.log、ui-0917-r3.err～r5.err；r6 最小回歸通過。Clippy 首輪只發現測試模組排列，已移到實作後方，未停用 lint。

## 發布前候選檢查

0.9.17 候選來源已通過 fmt、全目標 Clippy、492項非CAD Rust（60項CAD過濾）、80項前端、原生 UI 雙任務／排程名額、實際 Word／Excel 資源排隊、兩個 Python 沙箱與四項 panic 自檢。新負向改名測試在平行 Rust 測試中曾干擾其他 pin，已用同一短交易協調探針；最終完整測試通過。正式空快取 release 建置、候選安裝包、CI 與公開下載仍待後段收據，不以此段當成發布完成。
