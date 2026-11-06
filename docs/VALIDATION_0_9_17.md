# 0.9.17 驗證紀錄

M11 與兩項公司回饋修正已完成本機相關驗證、正式建置、封裝、Git／Release 發布及公開下載核對。公司真模型驗收由使用者進行。

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

## 發布與最終證據

- 來源與 v0.9.17 標籤：`3b6c5c1838e8a0c7cdbdd8e5996af51b4b1c8b76`；後續僅補文件／驗證收據，不移動標籤。
- scripts/Build.ps1 -BuildOnly -EmptyCargoCache：MSVC v142 14.29.30133、_MSC_FULL_VER=192930159、x64，Cargo.lock／vendor／.cargo/config.toml 齊備，空 Cargo 快取 frozen 成功。
- fmt、全目標 Clippy 及492項非CAD Rust通過；60項CAD過濾。兩項桌面通知測試在本機實際桌面通過。最後平行測試曾被新增負向改名探針干擾，探針納入短交易協調後全數通過，原失敗留在 .build/test-0917-final-r1.log。
- 80項前端通過：雙任務15、分支19、計畫14、對話32；亮／暗／650px畫面已檢查，任務卡片不溢出。
- 正式EXE六組原生UI收據通過。雙任務包括實際loopback runner執行緒啟動與取消／Finished，以及受控卡片歸屬、過期停止拒絕、排程共用名額與全域quarantine。
- 真實Word A／B依序開啟、保存與讀回；A占Word時Excel獨立讀寫；外部Word仍回既有BUSY。Outlook與CAD鎖已接入，但真實公司信箱／CAD圖面未實測。
- 兩個Python AppContainer分別持有獨立Job與1GiB限制；合成64MiB陣列測得程序尖峰約141.77／144.76MiB，單方取消不影響另一方。兩個任務若各跑兩項batch，最多4個Python／4GiB上限，另計主程式及Office／CAD，不代表公司大型資料尖峰。
- 四項獨立原生panic自檢（背景、WndProc、WebView2、abort持久化）通過；兩任務quarantine的控制器回歸另驗，不冒充公司長任務壓力測試。
- Excel筆記成功來源登記、舊封存補登記、錯誤ID拒絕、科學數值／分塊封存精確讀回與真正竄改拒絕通過。
- 實際Setup安裝／更新／啟動／解除安裝、WebView2缺失提示、EXE／Python／CAD runtime、runtime身分及使用者資料保留通過。安裝器未修改，完整PID／回復／修復矩陣沿用0.9.15。
- [同來源CI](https://github.com/you1230783-sys/CompanyAI/actions/runs/42159015062)通過，下載CI報告確認v142、空快取frozen及依賴公告；有期限ttf-parser維護公告例外保留。
- [v0.9.17 Release](https://github.com/you1230783-sys/CompanyAI/releases/tag/v0.9.17)五項公開下載大小及SHA256均一致。
- EXE SHA256：`30cadc769eed4318e58c402ed59c6a6d33394b35aa9e05231ae97cc12f8a82d1`。
- Setup SHA256：`a3455d8658c2797ed03c5265923db144d8ab2427dcc8a75aa814e6912514185e`。

證據：[發行](../offline/release-verification-0.9.17.json)、[正式建置](../offline/exe-verification-0.9.17.json)、[安裝包](../offline/installer-verification-0.9.17.json)、[資源並行](../offline/resource-verification-0.9.17.json)、[原生控制器](../offline/composer-verification-0.9.17.json)、[panic](../offline/reliability-verification-0.9.17.json)、[前端](../offline/frontend-verification-0.9.17.json)、[CI](../offline/github-ci-verification-0.9.17.json)、[公開下載](../offline/release-download-verification-0.9.17.json)。

安裝後有一次自檢超過既有45秒上限；同一EXE未改碼、未放寬門檻，重驗約21.69秒通過。其後從同一受控安裝接續重驗安裝資料、更新、啟動及解除安裝；失敗與接續紀錄分別保留 .build/release-0.9.17-r1.log、r2.log 與 installed-ui0917-retry1.json。原因未完全確認，不將首次逾時寫成成功。

## 公司端待驗與停止界線

公司模型、長任務EIS、真實Excel／信箱／CAD／SMB與實際排程仍由使用者驗收。本機排程驗的是控制器名額與合成固定流程，未用真實公司排程。未重跑完整科學分析與Office文件矩陣。0.9.16附件跨網站對話仍受後端授權限制；失敗時須在新對話重新附加。GitHub提供手動下載，未部署公司更新伺服器。

本輪到0.9.17完成後停止，等待公司回饋，不實作0.9.18。兩份手冊與FUTURE_IDEAS未修改。
