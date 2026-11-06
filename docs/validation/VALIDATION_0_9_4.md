# 0.9.4 驗證與發行紀錄

日期：2026-11-04。狀態：本機完整v142空快取建置、Office與NSIS生命週期驗證通過；發布及公開下載核對由交付流程接續完成。公司實機仍待驗。

## 本版範圍

本版是「兩項小修＋純搬移」，不能宣稱整版完全沒有行為變更。先修正、驗證並提交，再固定拆分基準；沒有改Windows套件、Cargo features、LTO或依賴版本。

- P01，`91a1797`：一般批次工具錯誤仍是單項失敗；內部panic經真正的全域hook寫入診斷並進入保護狀態。通知同組工作取消，等待執行緒結束後停止本組成果提交，不啟動下一組，不重播先前成功操作。使用既有guarded處理panic payload，沒有暫時關閉或替換全域hook。
- P02，`b0a7374`：Word比較送出前明示段落位置對應，以及前方插入／刪除可能造成大量差異；不改比較演算法。
- M01：`aa36296`拆分files、`74d80d1`拆分ui、`146199f`拆分runner。每個commit有函式原位置→新位置清單，詳見[搬移對照](../history/REFACTOR_0_9_4.md)及三份`docs/refactors/0.9.4-*.json`。
- `b739fce`只移除拆出測試模組開頭的空白行；核對工具新增／改善也分開提交，沒有混入純搬移commit。

## 搬移核對

| 原檔 | 搬移前→後行數 | 搬移函式 | 留在原檔的函式 |
| --- | ---: | ---: | ---: |
| src/projects/files.rs | 4844→1663 | 41 | 63 |
| src/ui.rs | 2645→594 | 27 | 17 |
| src/projects/runner.rs | 1932→287 | 19 | 18 |

87個搬移函式與98個保留函式的本體核對通過。工具使用固定rustfmt正規化，比較保留字串、字元及註解的詞元；只接受清單內的模組引用／include_str路徑調整，不直接忽略字串空白或刪除逗號。公開入口維持原路徑，以重匯出保持相容。

可見性調整限原父層作用域；UI的ComApartment仍供ui層使用。App、Broker及暫停資料的型別與序列化欄位仍留在原模組。核對工具不能取代可見性／型別的人工審查、編譯及實際回歸；本版仍執行完整驗證。

大型`run_for`及工具分派match保留完整控制流程，僅移到專責檔案；沒有趁拆分改成新抽象或重排執行順序。panic的來源檔案／行號隨搬移改變，診斷仍帶版本與位置。

## 定向測試

- 新增真hook批次回歸，以同一Rust測試二進位的獨立子程序執行，避免全域保護狀態污染其他測試。只在測試編譯配置注入內部panic，正式成品沒有此分支或可由檔案／環境開啟的注入入口。
- 實際broker批次涵蓋：普通失敗仍可回報其他成功項、panic留下同步且不含payload的紀錄、同組不提交、下一組不啟動、已封存操作身分保留、保護狀態再次呼叫不重播。
- files拆分後494項Rust測試通過；ui與runner分別通過v142全部目標編譯。最終完整流程亦已再次通過494項測試。
- 文件介面21項（含Word比較限制可見）、工作入口21項、對話20項、草稿18項、登入／20種對話框、可靠度18項及設定／57主題教學通過。
- 原始證據留在本機`.build/batch-panic-094-r2.log`、`.build/source-moves-094-final.log`與`.build/Test-*-094.log`；公開摘要不包含Windows帳號或私人設定。

## 最終完整流程

使用`scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller -TestOffice`，完整原始日誌為`.build/build-0.9.4-full-r2.log`。

| 驗證 | 結果 |
| --- | --- |
| 工具鏈／離線 | PASS；Rust 1.98.1、MSVC v142 x64 14.29.30133、實際編譯器192930159、SDK 10.0.19041.0，空Cargo快取與vendor frozen建置 |
| Rust／lint | PASS；494項、fmt、Clippy全部目標，包含真hook批次panic獨立子程序 |
| 原生工具／恢復／圖片 | PASS；28組代理、9組恢復、14組圖片；包含checkpoint中斷後恢復 |
| WebView2／可靠度 | PASS；13項工作區／匯出／提醒、9項對話交接及既有編輯器自檢；背景、WndProc、WebView2／COM與abort紀錄 |
| Office／文件工作 | PASS；10種修訂格式、新建Word／Excel／PPT、圖片嵌入、8組翻譯／比較／匯出／MD、最終OOXML及PDF畫面核對 |
| Python／資料／隔離 | PASS；實際Excel→CSV→pandas、平行批次、120秒逾時、取消、來源版本、檔案／網路／子程序隔離與資料讀回 |
| CAD | PASS；101項合成契約、原生runtime與安裝自檢；未重新做公司實際圖面或模型驗收 |
| NSIS | PASS；固定路徑、新裝、PID交接、升級／重啟、失敗回復、Python修復、CAD清理保護、正式成品安裝與移除；保留使用者資料 |

公開[發行摘要](../../offline/release-verification-0.9.4.json)索引23份本版驗證JSON。批次測試使用真broker與真hook、測試專用計算輸入；沒有將其宣稱為真模型驗證。

最終文件檢查使用本次完整流程新建的固定範例。OOXML核對原樣式副本圖片位元組、非空頁首及表格數；匯出Word含真正表格與原PNG。PDF經Windows引擎轉成頁面，人工確認中文標題、表格、程式碼、來源網址及圖片可讀；PDF雜湊記於文件成品報告。這不代表任意文件版面或翻譯語意都已驗收。

## 開發中修正的測試／搬移問題

1. 新批次回歸起初沒有啟用broker記憶體，無法封存操作；補齊真實批次所需的記憶體初始化，並核對封存索引而非已清空的暫存結果。沒有放寬正式保存流程。
2. files測試拆檔後，`include_str!("tools.json")`必須改為`../tools.json`；UI測試資源同樣調整一層目錄，資源本身未變。
3. UI的ComApartment搬到bootstrap後，需要保留ui層可見範圍供教學入口使用；設定pub(super)，沒有擴成公開API。
4. 原始詞元比對捕捉到rustfmt因路徑變長而新增尾逗號；改用同一固定rustfmt正規化兩邊，再逐詞元比對，避免把`(x)`和`(x,)`錯當相同。
5. 第一輪完整流程在格式檢查即停止：新測試模組多一個開頭空白行。移除並再次確認格式後，以第二輪完整流程驗證，不把第一輪記成PASS。

## 不變條件與待驗範圍

- 安裝固定`C:\largan\LM_AI`；公司HTTP、lm_server及machines.json明文格式／匯入匯出不變。
- 更新同源、RSA與SHA256檢查不放寬。GitHub只供手動下載，本輪不部署公司伺服器。
- 原件唯讀、CAD副本、成功寫入不重播、帳號隔離與私有資料不提交的規則保留。
- Cargo.lock、vendor與`.cargo/config.toml`維持離線建置契約。本版Cargo.toml／lock只改應用版本，不改依賴、features或release最佳化設定。
- Git不恢復LFS或建置二進位，不改寫0.8.75起點。NAS同步由使用者自行設定，本輪不搬動共享工具或備份資源。
- 公司登入／模型／翻譯品質、加密Office、真Classic Outlook信箱、SMB／VNC／防毒、多螢幕DPI與桌寵人工操作仍需公司確認。
- Word目前仍按段落位置比較；內容對齊排在LTO之後的下一版。後續依序為Windows綁定、Cargo features、CI／漏洞檢查、LTO、Word比較，再保留評價／統計與無人值守排程。

## 發行

[0.9.4手動下載入口](https://github.com/you1230783-sys/CompanyAI/releases/tag/v0.9.4)提供NSIS、兩份相容更新JSON、verification.json與SHA256SUMS.txt；單獨EXE只留本機。

| 成品 | bytes | SHA256 |
| --- | ---: | --- |
| LM_AI_Setup.exe | 55520657 | `23069e6823b8ac8d4ef7da12bc33cab79af5116529e7e18316189877cdbac687` |
| LM_AI.exe（僅本機） | 69492736 | `e97e8e28dfde02cf9ff80e8b16be36fb7a64fdd76a89af63c755049b1276991f` |

程式內更新仍從公司`/lm_server/desktop/releases/LM_AI_Setup.exe`下載，本輪未部署公司伺服器。發布後不帶憑證下載五項公開資產，逐一核對大小與SHA256；證據留在本機`.build/release-download-verification-0.9.4.json`，於交付回覆確認外部發布結果。
