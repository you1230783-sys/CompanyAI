# 0.9.5 驗證與發行紀錄

狀態：本機完整v142空快取建置、Office與NSIS生命週期驗證通過；發布及公開下載核對由交付流程接續完成。

## 本版範圍

- M04a評估後保留兩套Windows綁定，理由與分工見[WINDOWS_BINDINGS.md](../development/WINDOWS_BINDINGS.md)。不是刪除windows-sys，也未改寫Win32函式簽章。
- 將Office、Outlook、WebView2／教學、WIC與UI Automation的COM初始化及釋放集中到單一守衛。守衛不可跨執行緒，只有本次成功初始化才釋放；S_FALSE同樣平衡，WIC借用STA不釋放原持有者。
- UI Automation改由守衛處理正常／提前離開與unwind。COM物件仍須先於守衛釋放；未將Windows訊息泵、錯誤碼或權限行為混入改動。
- 本版不修改Cargo features、依賴版本、LTO、Word比較或遙測，不宣稱體積優化。

## 定向驗證

`com_apartment::tests::real_apartment_lifecycle_in_isolated_process`通過，包含五種真COM生命週期情境。原始日誌保存在本機`.build/com-095-test.log`。測試隔離程序不接觸公司服務、信箱或使用者文件。

## 執行環境與重跑紀錄

第一輪`.build/build-0.9.5-full-r1.log`在Rust測試停止：494項通過，Windows Shell未接受測試托盤圖示；沒有修改測試斷言。另一次受限環境的Edge測試未產生DevTools連接檔。以隔離測試profile在原生測試環境重跑七組UI檢查通過，完整流程改以第二輪日誌為準；第一輪不列PASS。Git維護子程序曾回報受限環境的signal pipe錯誤，但commit本身成功，提交與工作目錄另行核對。

## 不變條件與公司待驗

安裝固定`C:\largan\LM_AI`；公司HTTP、lm_server、machines.json明文格式／匯入匯出、更新同源／RSA／SHA256均不變。GitHub只供手動下載，本輪不部署公司伺服器。原件唯讀、CAD副本、成功寫入不重播、私有資料不提交的約定保留。

公司登入與模型、翻譯品質、加密Office、真Classic Outlook信箱、SMB／VNC／防毒、多螢幕DPI與桌寵人工操作仍待公司確認。NAS同步由使用者處理，本輪不搬動共享工具或備份。

## 完整流程與發行


使用`scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller -TestOffice`，完整原始日誌為`.build/build-0.9.5-full-r2.log`。

| 驗證 | 結果 |
| --- | --- |
| 工具鏈／離線 | PASS；Rust 1.98.1、MSVC v142 x64 14.29.30133、實際編譯器192930159、SDK 10.0.19041.0，空Cargo快取與vendor frozen建置 |
| Rust／lint | PASS；495項、fmt、Clippy全部目標，包含真COM生命週期及既有真hook批次panic獨立子程序 |
| 原生工具／恢復／圖片 | PASS；28組代理、9組恢復、14組圖片；包含checkpoint中斷後恢復 |
| WebView2／可靠度 | PASS；13項工作區／匯出／提醒、9項對話交接及既有編輯器自檢；背景、WndProc、WebView2／COM與abort紀錄 |
| Office／文件工作 | PASS；10種修訂格式、新建Word／Excel／PPT、圖片嵌入、8組翻譯／比較／匯出／MD、最終OOXML及PDF畫面核對 |
| Python／資料／隔離 | PASS；實際Excel→CSV→pandas、平行批次、120秒逾時、取消、來源版本、檔案／網路／子程序隔離與資料讀回 |
| CAD | PASS；101項合成契約、原生runtime與安裝自檢；未重新做公司實際圖面或模型驗收 |
| NSIS | PASS；固定路徑、新裝、PID交接、升級／重啟、失敗回復、Python修復、CAD清理保護、正式成品安裝與移除；保留使用者資料 |

公開[發行摘要](../../offline/release-verification-0.9.5.json)索引23份本版驗證JSON。新增COM子程序測試包含五種真實初始化／釋放情境；沒有將記憶體COM測試宣稱為真Outlook信箱驗收。

最終文件檢查使用本次完整流程新建的固定範例。OOXML核對原樣式副本圖片位元組、非空頁首及表格數；匯出Word含真正表格與原PNG。PDF經Windows引擎轉成頁面，人工確認中文標題、表格、程式碼、來源網址及圖片可讀；PDF雜湊記於文件成品報告。這不代表任意文件版面或翻譯語意都已驗收。

七組前端回歸通過：文件21項、工作入口21項、對話20項、草稿18項、登入／20種對話框、可靠度18項、設定與57主題教學。

## 發行

[0.9.5手動下載入口](https://github.com/you1230783-sys/CompanyAI/releases/tag/v0.9.5)提供NSIS、兩份相容更新JSON、verification.json與SHA256SUMS.txt；單獨EXE只留本機。

| 成品 | bytes | SHA256 |
| --- | ---: | --- |
| LM_AI_Setup.exe | 55531413 | `62cbdc32a4137e28b48880608d3bfe879daa71d1c1269dfed5c0b2a5d1184493` |
| LM_AI.exe（僅本機） | 69522944 | `30538ee233def3e6bfeb01a754ce53320fbcd541909e1ab90f5646a3867d5a61` |

程式內更新仍使用公司`/lm_server/desktop/releases/LM_AI_Setup.exe`，本輪未部署公司伺服器。公開下載五項資產的SHA256／大小核對由交付流程執行，證據留在本機`.build/release-download-verification-0.9.5.json`。
