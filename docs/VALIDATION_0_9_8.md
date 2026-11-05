# 0.9.8 驗證與發行紀錄

本機完整驗證與必要封裝已通過。GitHub同提交CI及公開下載結果由本版Release與發行後交接紀錄列出；本機結果不能代替遠端驗證。

本版D02b選用ThinLTO／16 codegen units，明確保留panic unwind、MSVC v142 x64、靜態CRT及既有功能。配置比較、來源與可重跑入口見[建置配置](RELEASE_PROFILES.md)及[量測JSON](../offline/profile-measurements-0.9.8.json)。本版不含F08／F09／C01；它們在0.9.9提交來源後，依最新使用者指示合併到0.9.10驗證及發布。

## 來源與命令

應用來源提交：`d9b9d1e743052f595ba3b16d13b70d1a4d3b381b`。之後的本版證據提交只新增驗證資料與文件，不變更已驗證的應用、資源或建置設定。

- `scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller -TestOffice`：本機完整建置及整合，原始日誌`.build/build-0.9.8-full-r1.log`。
- 核心配置另以v142及新空Cargo快取執行`cargo clippy --workspace --all-targets --no-default-features --frozen -- -D warnings`及`cargo test --workspace --no-default-features --frozen`；日誌`.build/core-098.log`。
- 預設配置的fmt／Clippy／499項Rust結果直接沿用同來源完整Build，不重跑。本機未再次執行Test-CI.ps1；[預設／核心合併證據](../offline/windows-ci-verification-0.9.8.json)明列兩個來源。GitHub仍以Test-CI.ps1獨立核對發行提交。
- 九組前端回歸：Test-DocumentWork、Test-Workbench、Test-Conversations、Test-DraftTools、Test-UiAuth、Test-ReliabilityUI、Test-Settings、Test-PetTarot、Test-BuildFeatures；使用scripts內相應cjs入口。
- `scripts/Test-DependencyAudit.ps1`：刷新RustSec並執行既有12項政策案例。

## 實測結果

| 範圍 | 結果 |
|---|---|
| 工具鏈／離線 | PASS；Rust1.98.1、v142 14.29.30133、實際192930159 x64、SDK10.0.19041.0；Cargo.lock／vendor／專案.cargo及空快取frozen |
| Rust／lint | PASS；完整499、核心455、fmt及兩配置all-targets Clippy；無本機互動桌面略過 |
| Release panic | PASS；背景、原生回呼、COM與abort前持久紀錄，使用選定最佳化配置的真實獨立程序 |
| 原生代理／恢復／圖片 | PASS；28組代理、9組恢復、14組圖片，以及跨批次／中斷／未知提交／取消／操作去重 |
| 檔案／Python | PASS；AppContainer、來源與版本保護、Big5、大型LOG、真平行批次、120秒逾時與取消、CSV／XLSX讀回 |
| Office／文件 | PASS；10種格式修訂、三種新建文件、PNG保存重開、8組翻譯／比較／匯出／Markdown；真Excel→CSV→pandas、固定計算值及已保存流程重跑 |
| 文件結構／畫面 | PASS；本次固定範例的OOXML圖片／頁首／表格與PDF畫面核對；Windows原生PDF引擎轉PNG後檢查 |
| CAD | PASS；101項合成契約與runtime自檢；未重新驗收公司圖面或真模型 |
| 前端／原生UI | PASS；九組來源頁面回歸（含131項feature介面檢查）及完整Build內WebView2／工作區／提醒／交接自檢 |
| NSIS | PASS；新裝、固定位置、PID交接、升級／重啟、失敗回復、Python修復、CAD清理保護、正式成品安裝與移除，保留使用者資料 |
| 依賴政策 | PASS；無已公告漏洞或撤回版本，保留ttf-parser精確且有期限的停止維護例外，不宣稱零警告 |

公開[發行摘要](../offline/release-verification-0.9.8.json)索引28份證據。`simulated interruption`是測試主動注入並驗證恢復的panic，不是被忽略的失敗。首次sandbox Git提交曾輸出shell signal-pipe警告，但提交成功；後續需權限的Git／網路及原生流程使用已核准執行環境。

固定PDF範例的中文、表格、程式碼、來源URL、紅藍PNG與頁尾可讀且未裁切；既有匯出仍保留Markdown圍欄／連結／圖片記號，不能宣稱完整Markdown排版或任意文件版面已驗收。翻譯語意、公司模型／信箱、公司加密Office、實際SMB／VNC／防毒、多螢幕DPI與桌寵拖曳仍待公司實機確認。

## 成品與發行邊界

| 成品 | bytes | SHA256 |
|---|---:|---|
| LM_AI_Setup.exe | 55386691 | `2b733c7f6c1c104a34d725b6545f845b7d1ed782bd6eab879a3d6e950debaf4f` |
| LM_AI.exe（僅本機） | 68927488 | `c2a9b6b4456576c76c88f3aacdca392dde3f02550872a6e9cfa2de8894659f79` |

兩份相容更新JSON均指向NSIS，已由正式EXE核對RSA、SHA256與版本。公司更新仍使用`/lm_server/desktop/releases/LM_AI_Setup.exe`，沒有部署公司伺服器；GitHub僅提供手動下載。Release資產限定安裝包、兩份更新JSON、verification.json與SHA256SUMS.txt。


## 已發布收據（2026-11-05）

已發布 [v0.9.8](https://github.com/you1230783-sys/CompanyAI/releases/tag/v0.9.8)，tag 與發布時 main 同為 `2b6ece35204b922dda2a87988eb7d4b82ec646e7`。
[同提交 CI](https://github.com/you1230783-sys/CompanyAI/actions/runs/41959105102) 通過，下載報告確認 v142／空快取 frozen／完整與核心 lint、Rust 及依賴政策。
證據提交內描述後續 0.9.9 節奏時帶入 CI 跳過標記，故改以 workflow_dispatch 在同一提交執行完整既定 CI；沒有免除任何發布檢查，也沒有改寫歷史。
[遠端報告收據](../offline/github-ci-verification-0.9.8.json)及[五項公開資產下載核對](../offline/release-download-verification-0.9.8.json)皆 PASS。
本節為發布後文件收據，不變更已測程式或成品；0.9.9 依最新授權只提交原始碼，完整驗證與封裝合併到 0.9.10。
