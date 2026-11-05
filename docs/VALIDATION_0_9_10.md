# 0.9.10 合併驗證與發行紀錄

本機完整v142驗證與必要封裝已通過。GitHub同提交CI、Release及公開下載由發行後收據補記；本機結果不能代替遠端驗證。

本版合併0.9.9 F08／F09／C01與0.9.10 D03。0.9.9僅推送來源`b3bbfe9`，依指示沒有單獨編譯、驗證、封裝或Release。實作界線見[0.9.9來源交付](IMPLEMENTATION_0_9_9.md)及[固定流程排程](SCHEDULED_WORKFLOWS.md)。

## 來源與重跑入口

應用來源提交：`231c313be8c87386991b7f7691ed15fb6dc29c98`。後續證據提交只新增文件及驗證資料，不變更已驗證的應用、資源或建置設定。

- `scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller -TestOffice`：原始日誌`.build/build-0.9.10-full-r5.log`；第21組CSV舊斷言失敗後，僅修改測試example，以相同應用來源與EXE SHA256接續案例21–27及所有後續Build階段。接續日誌`.build/build-0.9.10-continuation-r3.log`，合併索引`.build/build-0.9.10-combined.log`；不是單次完整Build零退出。
- 核心配置另外在v142、新空Cargo快取執行`cargo clippy --workspace --all-targets --no-default-features --frozen -- -D warnings`及`cargo test --workspace --no-default-features --frozen`；日誌`.build/core-0910.log`。
- 接續入口為`project_smoke.exe <同一EXE> <新測試目錄> --native-only 21`及`--after-native`，之後按Build.ps1自`python_smoke`起執行全部剩餘步驟；未跳過Office、排程或NSIS。原腳本SHA256、應用來源比對範圍與EXE SHA256記於發行摘要的`execution_provenance`。完整重跑仍使用上方標準Build命令。
- 預設fmt／Clippy／511項Rust沿用同來源完整Build；本機沒有重跑Test-CI.ps1。[合併證據](../offline/windows-ci-verification-0.9.10.json)明列來源；GitHub對發行提交另執行Test-CI.ps1。
- 十組前端：scripts內Test-Schedules、Test-BuildFeatures、Test-DocumentWork、Test-PetTarot、Test-Settings、Test-Workbench、Test-Conversations、Test-DraftTools、Test-UiAuth、Test-ReliabilityUI的cjs入口。
- `scripts/Test-DependencyAudit.ps1`：12項政策案例與刷新RustSec。

## 實測結果

| 範圍 | 結果與界線 |
|---|---|
| 工具鏈／離線 | PASS；Rust1.98.1、MSVC v142 14.29.30133、實際192930159 x64、SDK10.0.19041.0；Cargo.lock／vendor／專案.cargo及空快取frozen |
| Rust／lint | PASS；完整511、核心467、fmt及兩配置all-targets Clippy；本機未排除互動桌面案例 |
| Release panic | PASS；真實release獨立程序驗證背景、原生回呼、COM及abort前持久記錄；ThinLTO／16 units／unwind |
| 原生代理／恢復／圖片 | PASS；28組代理、9組恢復、14組圖片；含134次HTTP的長流程、中斷恢復、取消、未知提交與成功操作去重 |
| F08 | PASS；Word插入／刪除、重複文字、表格旗標、純格式及邊界單元案例；原生Office比較回歸。不是任意文件的語意／移動段落對齊承諾 |
| F09 | PASS；舊設定預設關閉、四種偏好組合、編譯能力、教學直接入口及原生離線開關兩種狀態；VNC維持獨立 |
| C01 | PASS；完整請求估算50k軟門檻、超標整理至約40k、成對歷史移出及原生長流程；技能／筆記與安全去重保留。公司模型重讀率及累積用量尚未實測 |
| D03 | PASS；到期寬限、認領去重、DPAPI、授權版本與損毀狀態；真Broker／Worker保存副本、安全邊界續接、送出前／保存後中斷拒絕重播及UI預覽確認 |
| 檔案／Python | PASS；AppContainer、來源／版本保護、Big5、大型LOG、真平行批次、120秒逾時與取消、CSV／XLSX讀回 |
| Office／文件 | PASS；格式修訂、三種新建文件、PNG保存重開、翻譯／比較／匯出／Markdown、真Excel→CSV→pandas及保存流程重跑 |
| 文件結構／畫面 | PASS；本次固定範例OOXML圖片／頁首／表格與PDF轉PNG畫面核對；不是完整Markdown版面保真 |
| CAD | PASS；101項契約與runtime自檢；公司圖面與真模型未重新驗收 |
| UI／NSIS | PASS；十組前端及原生WebView2、工作區、交接；NSIS新裝、固定位置、PID交接、升級／重啟、失敗回復、Python修復、CAD清理保護及移除 |
| 依賴政策 | PASS；無已公告漏洞或撤回版本；ttf-parser有精確且有期限的停止維護例外，不宣稱零警告 |

公開[發行摘要](../offline/release-verification-0.9.10.json)索引29份證據。`simulated interruption`為主動注入並核對恢復的測試panic，未忽略真正失敗。

## 驗證期間發現與修正

1. 舊測試引用已移除的bytes常數：改核對完整原生請求50k／40k語意，保留較早回答與明確恢復節錄的區別；修正Clippy單分支與無用unit。
2. 原生排程fixture需要唯讀`directory_identity`，將原函式改為公開；沒有新增模型工具或UI任意路徑能力。
3. 新增設定後白名單欄位數由19改20，並明確檢查額外功能預設false；Settings的57項教學fixture明確開啟額外功能及VNC。
4. r4的511項Rust通過，但原生自檢仍假設離線桌寵／塔羅永遠顯示，導致完整Build停止。修正為先驗證額外功能關閉時隱藏，再驗證開啟時尊重編譯能力；沒有改產品預設。r5通過此項及原生案例0–20，未把r4當作完整PASS。
5. r5第21組仍要求CSV匯出立即移出原文，與C01不符。改驗證預算內保留、明確整理後移出、100筆CSV與圖表正確及原文按需查回；首次接續的固定工具數未包含既有自動筆記，修正為至少保留讀取／技能／匯出結果；第二次接續發現仍要求舊版CSV自動整理通知，移除該通知斷言，保留實際CSV、圖表及原文查回檢查。僅修改examples，保留相同應用與EXE；r3接續重新執行案例21–27、其餘專案群組及全部後續Build階段成功。新`--after-native`只分組既有測試，完整Build預設仍執行全部群組。前述失敗與接續均保留於證據，不宣稱r5單次完整通過。

## 成品與待公司驗收

| 成品 | bytes | SHA256 |
|---|---:|---|
| LM_AI_Setup.exe | 55929478 | `2ce5818e2cfc51b5597b2af20f8c57276a15fa4678423dedff84f1ce1200d0c8` |
| LM_AI.exe（僅本機） | 70714880 | `e9c16017e28ef67757080bf8982a7b2f022d2f6b88bad13cb07e3cdc35de6fac` |

兩份相容更新JSON均指向NSIS，由正式EXE核對RSA、SHA256與版本。公司更新仍為`/lm_server/desktop/releases/LM_AI_Setup.exe`；公司HTTP及固定安裝位置不變，沒有部署公司伺服器。GitHub資產限定安裝包、兩份更新JSON、verification.json及SHA256SUMS.txt。

公司模型重讀頻率／累積用量／完成品質、真Outlook信箱、加密Office、公司SMB／VNC／防毒、人工框選／多螢幕DPI及桌寵拖曳待實機確認。排程僅程式開啟／托盤有效；休眠／鎖屏長時間驗收未完成。需互動同意、品質選擇或渲染器的流程可能暫停／失敗；未知寫入不能重播。固定PDF範例可讀但仍保留Markdown圍欄／連結／圖片記號。

Claude二次驗證由使用者在本版完成後安排，目前沒有宣稱已通過。後續版本由實測回饋另行決定，本輪止於0.9.10。
