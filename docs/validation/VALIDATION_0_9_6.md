# 0.9.6 驗證與發行紀錄

狀態：本機v142空快取建置、完整回歸、Office與NSIS生命週期通過；發布及公開下載核對由交付流程接續完成。

## 本版範圍

M04b：四個獨立Cargo features，預設完整；原生拒絕邊界、介面能力、舊資料保留與開發建置方式見[CARGO_FEATURES.md](../development/CARGO_FEATURES.md)。PNG預覽helper先以獨立純搬移commit `3ef6244`移至核心；搬移1項、保留3項函式內容核對通過。

## 定向驗證與修正紀錄

- 核心版編譯初次發現大型json巨集深度、停用模組殘留引用與CAD清理回報型別問題，修正後`cargo check --frozen --all-targets --no-default-features`通過。
- 核心Rust首輪449項通過、2項失敗：舊測試假設工具目錄包含CAD，以及CAD比較匯出測試假設可沿用成功結果。更新測試能力範圍並增加原生命令、停用資產與舊紀錄拒絕案例後，453項通過。完整版仍須跑全部原有CAD測試。
- 矩陣首輪Clippy指出測試模組位置及停用VNC後Command變體大小比例。測試移至檔尾；Project命令維持312 bytes、既有按值派送，僅在沒有VNC時註明允許比例警告，不引入無必要的堆積配置。
- 16種介面組合共131項檢查通過，包括入口、原VNC偏好保留、停用入口不送命令、CAD能力提示、教學與提醒。測試初次缺少回覆造成busy跨組延續，補上模擬回覆後通過，沒有放寬產品斷言。

## 不變條件與公司待驗

安裝固定`C:\largan\LM_AI`；公司HTTP、lm_server、machines.json明文格式及匯入匯出、更新同源／RSA／SHA256保持不變。GitHub僅為手動下載。原件唯讀、CAD副本、成功寫入不重播與私有資料不提交的規則繼續適用。

公司模型、真實公司信箱、加密Office、SMB／VNC實際機台、公司防毒與人工DPI仍須公司環境驗收；本機或模擬測試不取代這些項目。


## 選用組合實測

16種組合全部通過`--no-default-features`搭配選定features的frozen Clippy all-targets／`-D warnings`；核心453項Rust通過。矩陣先驗證feature實作，進版後再以核心及完整兩個0.9.6 EXE實際建置／驗證。

核心EXE另以`Build.ps1 -BuildOnly -NoDefaultFeatures -EmptyCargoCache`建置，v142實際探針與空快取frozen通過；原生WebView2、截圖10項、編輯器14項、工作區13項、交接9項及任務追蹤6項通過。CAD自檢／清理拒絕執行，兩種自訂features誤用正式封裝／驗證的入口也被拒絕。核心EXE僅為開發驗證，不上傳Release。

功能介面矩陣131項及完整版桌寵／塔羅回歸通過。停用能力、保留VNC偏好、提醒替代方式與教學篩選均有證據；停用資產與原生命令另由Rust測試核對。完整版本仍包含全部選用模組，不宣稱完整版體積已優化。

## 最終完整流程


使用`scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller -TestOffice`，完整原始日誌為`.build/build-0.9.6-full-r1.log`。

| 驗證 | 結果 |
| --- | --- |
| 工具鏈／離線 | PASS；Rust 1.98.1、MSVC v142 x64 14.29.30133、實際編譯器192930159、SDK 10.0.19041.0，空Cargo快取與vendor frozen建置 |
| Rust／lint | PASS；497項、fmt、Clippy全部目標，包含真COM生命週期及既有真hook批次panic獨立子程序 |
| 原生工具／恢復／圖片 | PASS；28組代理、9組恢復、14組圖片；包含checkpoint中斷後恢復 |
| WebView2／可靠度 | PASS；13項工作區／匯出／提醒、9項對話交接及既有編輯器自檢；背景、WndProc、WebView2／COM與abort紀錄 |
| Office／文件工作 | PASS；10種修訂格式、新建Word／Excel／PPT、圖片嵌入、8組翻譯／比較／匯出／MD、最終OOXML及PDF畫面核對 |
| Python／資料／隔離 | PASS；實際Excel→CSV→pandas、平行批次、120秒逾時、取消、來源版本、檔案／網路／子程序隔離與資料讀回 |
| CAD | PASS；101項合成契約、原生runtime與安裝自檢；未重新做公司實際圖面或模型驗收 |
| NSIS | PASS；固定路徑、新裝、PID交接、升級／重啟、失敗回復、Python修復、CAD清理保護、正式成品安裝與移除；保留使用者資料 |

公開[發行摘要](../../offline/release-verification-0.9.6.json)索引26份本版驗證JSON。既有COM子程序測試包含五種真實初始化／釋放情境；沒有將記憶體COM測試宣稱為真Outlook信箱驗收。

最終文件檢查使用本次完整流程新建的固定範例。OOXML核對原樣式副本圖片位元組、非空頁首及表格數；匯出Word含真正表格與原PNG。PDF經Windows引擎轉成頁面，人工確認中文標題、表格、程式碼、來源網址及圖片可讀；PDF雜湊記於文件成品報告。這不代表任意文件版面或翻譯語意都已驗收。

既有七組核心前端回歸通過：文件21項、工作入口21項、對話20項、草稿18項、登入／20種對話框、可靠度18項、設定與57主題教學。

## 發行

[0.9.6手動下載入口](https://github.com/you1230783-sys/CompanyAI/releases/tag/v0.9.6)提供NSIS、兩份相容更新JSON、verification.json與SHA256SUMS.txt；單獨EXE只留本機。

| 成品 | bytes | SHA256 |
| --- | ---: | --- |
| LM_AI_Setup.exe | 55531082 | `b4a49d7672a23ae67e661ecccdb9e0bc702c4d0aab8b70c3562518a92a474865` |
| LM_AI.exe | 69510656 | `f03a81e8c53c8ad7f73219f4f3a2b48e48cb1d9f5859a4329f746c490395757f` |

公司更新仍為`/lm_server/desktop/releases/LM_AI_Setup.exe`，未部署公司伺服器。公開下載核對另記於本機`.build/release-download-verification-0.9.6.json`。
