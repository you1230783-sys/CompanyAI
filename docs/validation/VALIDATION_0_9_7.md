# 0.9.7 驗證與發行紀錄

狀態：修正首次GitHub CI揭露的目錄鎖與跨語系測試問題後，本機v142空快取、完整回歸、Office與NSIS已重新通過；以下為r2新成品。發行提交的GitHub CI及五項Release公開下載SHA256已核對通過。

本版 D02a：v142 Windows CI、空快取 frozen 契約與依賴檢查，詳見 [CI 與依賴政策](../development/CI_AND_DEPENDENCIES.md)。唯一應用相依更新為已撤回的 yoke-derive 0.8.3 → 0.8.4。

初次漏洞掃描未發現已公告漏洞，發現一項撤回版本及一項停止維護公告。撤回版本修補後已重新掃描通過；ttf-parser 的精確維護例外、到期日與限制留在政策及原始報告，不宣稱零警告。

開發過程：cargo install 在隔離 registry 無法解析已發布的 cargo-audit 版本，改用官方固定 SHA256 預編譯工具。初次解壓路徑與官方 ZIP 內部目錄不符，已依實際目錄修正。這些失敗不記為通過。

漏洞政策另以固定資料測試正常結果、工具失敗、已審查公告、到期、新漏洞、不同版本、撤回、新公告、缺漏報告及隱藏篩選，共12項（另包含嚴重度及維護公告篩選拒絕）。到期案例初次揭露DateTime的本機時區轉換問題，改用DateTimeOffset明確比較UTC後通過。

不變條件：固定安裝路徑、公司 HTTP／lm_server、machines.json 明文及匯入匯出、公司更新同源／RSA／SHA256、原件唯讀及不重播成功寫入均保留。公司模型、公司信箱、加密 Office、SMB／VNC 實際機台、防毒與人工 DPI 仍須公司環境驗收。

## CI與依賴定向驗證

首次[GitHub CI](https://github.com/you1230783-sys/CompanyAI/actions/runs/41936574844)已成功安裝v142並完成frozen編譯，Rust為482通過、13失敗、2個互動桌面案例依政策排除。漏洞檢查通過既有精確維護公告政策。失敗不能由本機結果抵銷：

- 11項涉及專案記憶的原子替換回報共享衝突。記憶目錄鎖改用READ_ATTRIBUTES，避免不必要的DELETE存取需求；仍不分享DELETE，獨占anchor也保留。新增案例核對額外目錄讀取handle、初次發布／覆寫、拒絕目錄更名、拒絕anchor移除／寫入及目錄非空。本機C與Y槽原案例都能通過，Windows Server重驗結果見下段。
- 2項測試假定系統ANSI為BIG5。抽出明確碼頁的既有解碼邏輯，分別核對950、65001及1252歧義；LOG案例核對歧義必須回報來源錯誤及不完整結果。正式入口仍讀取GetACP，不放寬無損往返與拒絕歧義規則。

修正後已重新執行本機完整／核心CI及正式Office／NSIS流程，舊r1成品不發布。遠端重驗連結隨Release留存。

修正提交`05b10eb85821ef809cb952a2e411f1a8105d4363`的[遠端重驗](https://github.com/you1230783-sys/CompanyAI/actions/runs/41939340270)已通過：Windows Server 2022上的記憶替換及跨語系案例成功，未新增略過項目；下載CI報告確認v142 14.29.30133、實際192930159 x64、空快取frozen與依賴政策。本機r2及[最終發行提交CI](https://github.com/you1230783-sys/CompanyAI/actions/runs/41945624630)均已通過。

Test-CI.ps1在本機以空Cargo快取完成v142 release、fmt、完整／核心all-targets Clippy及499／455項Rust。本機沒有排除通知測試。Hosted模式的兩項排除見政策；實際遠端結果以對應提交的Actions執行為準。

RustSec資料庫掃描無已公告漏洞、無撤回版本；保留ttf-parser精確維護公告。原始JSON與政策摘要均收入公開證據，不能將政策通過讀成零警告。

## 正式流程


使用`scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller -TestOffice`，完整原始日誌為`.build/build-0.9.7-full-r2.log`。

| 驗證 | 結果 |
| --- | --- |
| 工具鏈／離線 | PASS；Rust 1.98.1、MSVC v142 x64 14.29.30133、實際編譯器192930159、SDK 10.0.19041.0，空Cargo快取與vendor frozen建置 |
| Rust／lint | PASS；499項、fmt、Clippy全部目標，包含真COM生命週期及既有真hook批次panic獨立子程序 |
| 原生工具／恢復／圖片 | PASS；28組代理、9組恢復、14組圖片；包含checkpoint中斷後恢復 |
| WebView2／可靠度 | PASS；13項工作區／匯出／提醒、9項對話交接及既有編輯器自檢；背景、WndProc、WebView2／COM與abort紀錄 |
| Office／文件工作 | PASS；10種修訂格式、新建Word／Excel／PPT、圖片嵌入、8組翻譯／比較／匯出／MD、最終OOXML及PDF畫面核對 |
| Python／資料／隔離 | PASS；實際Excel→CSV→pandas、平行批次、120秒逾時、取消、來源版本、檔案／網路／子程序隔離與資料讀回 |
| CAD | PASS；101項合成契約、原生runtime與安裝自檢；未重新做公司實際圖面或模型驗收 |
| NSIS | PASS；固定路徑、新裝、PID交接、升級／重啟、失敗回復、Python修復、CAD清理保護、正式成品安裝與移除；保留使用者資料 |

公開[發行摘要](../../offline/release-verification-0.9.7.json)索引28份本版驗證JSON（包含修正提交的GitHub CI下載證據）。既有COM子程序測試包含五種真實初始化／釋放情境；沒有將記憶體COM測試宣稱為真Outlook信箱驗收。

最終文件檢查使用本次完整流程新建的固定範例。OOXML核對原樣式副本圖片位元組、非空頁首及表格數；匯出Word含真正表格與原PNG。PDF經Windows引擎轉成頁面，人工確認中文標題、表格、程式碼、來源網址及圖片可讀；PDF雜湊記於文件成品報告。這不代表任意文件版面或翻譯語意都已驗收。

既有七組核心前端回歸通過：文件21項、工作入口21項、對話20項、草稿18項、登入／20種對話框、可靠度18項、設定與57主題教學。


另通過桌寵／塔羅與131項features介面回歸；本版重新執行完整／核心Rust與Clippy，沒有將0.9.6的16組編譯矩陣冒充本次結果。

## 發行

[0.9.7手動下載](https://github.com/you1230783-sys/CompanyAI/releases/tag/v0.9.7)僅提供NSIS、兩份相容更新JSON、verification.json與SHA256SUMS.txt；EXE留本機。

| 成品 | bytes | SHA256 |
|---|---:|---|
| LM_AI_Setup.exe | 55533469 | `5c75f7ab83028f160af778e3ca56f6ecf4f50250a61324e955d2103dd7ef19f6` |
| LM_AI.exe | 69511168 | `06d748324f32de32c82d0db86229cea5adb616289619ed325e4ff2ae48399907` |

公司更新仍為`/lm_server/desktop/releases/LM_AI_Setup.exe`，未部署公司伺服器。公開下載核對記於本機`.build/release-download-verification-0.9.7.json`。

發行完成：`v0.9.7`固定於`5fb110822ca461d92df867da48e57cd4c93a9751`；NSIS、兩份更新JSON、verification.json及SHA256SUMS.txt皆已用無授權標頭的公開下載重新核對大小與SHA256。之後main僅更新進度／交接文件，未重新編譯或更換成品。依使用者要求本輪到此停止。
