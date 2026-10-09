# 0.8.50：程式需求驗收與 Outlook 副本比對

## 行為

Outlook 使用主旨、SentOn 寄送時間、寄件地址及 To/CC 地址集合，在本機建立 SHA256 指紋。地址去除首尾空白並忽略 ASCII 大小寫，收件集合排序、去重且保留 To/CC 角色；BCC 不作共同指紋。時間統一使用寄送時間，不混用收件時間。完全相同四欄位依使用者決定視為副本；SHA256 是雜湊，不是加密。索引不讀 Message-ID、隱藏內文／附件或額外地址簿屬性，也不把隱藏欄位交給 AI。

欄位讀不到不再一律停止全部候選：可用欄位已足以證明不同的郵件可讀，其餘不確定候選暫時排除並回報不完整。隱藏項目的四欄位完全不可讀、資料夾無法列舉或掃描超過界線，仍明示查詢失敗、數量未知，不冒充零封。保持原有祖先授權及每次重建索引。Exchange 原地址可直接比較；不同資料檔若把同一人存成不同地址表示，不承諾能自動轉換成相同身分。

程式修改先以 `plan_code_change` 保存 user／preserve／added 需求。每段修改立即更新同一草稿；`check_python` 只驗證語法與編碼，不執行受檢來源。`review_code_change` 提供真實副本相對原版的差異摘要，再逐項記錄核對行號、證據及 tested／reviewed／unverified／incomplete。摘要有界線，必要時按函式回讀。incomplete 不發布為完成成果。

`test_python` 執行本次 revision 的完整來源快照，使用 unittest 與 mock；未載入來源、沒有案例、跳過案例均不能當通過。每批1–4組、每組12000 bytes、每版本最多24組，以原 id 重測更新結果。新的 AppContainer／Job 限制60秒、1GiB、單程序，私有暫存目錄支援合成檔案，禁止專案原件與網路；退出清除環境。缺少依賴或執行器無法完成回報 unavailable，不冒充通過。mock 的範圍必須列出，不等於真實外部服務已驗證。

語法及核對證據與版本綁定，任何修改均失效；通過的測試須對應需求才能引用。仍有失敗測試的需求不能改標 reviewed 繞過。缺外部环境可明列未驗證後交付，原生最終回覆會附核對及未通過案例。這些機制可重用於其他語言，但目前只有 Python 的執行工具。

## 驗證狀態

2026-10-09 15:18（Asia/Taipei）完整發行驗證通過。

- Rust 1.98.1、MSVC v142 14.29.30133、Windows SDK 10.0.19041.0；實際探針 `_MSC_FULL_VER=192930159 x64`。使用空 Cargo 快取、Cargo.lock、vendor 與專案 `.cargo/config.toml`，建置全程 `--frozen`。
- fmt、Clippy、299 項單元測試、正式 release EXE 與 WebView2 自檢通過。
- Outlook COM fixture 驗證 50 封中有 8 封隱藏副本後剩 42 封，以及分別 80→70、120→110；沒有 Message-ID／郵件 PropertyAccessor／Body 可用仍能比對。部分欄位缺失時保留其他 31 封無關郵件，不確定候選拒絕，四欄位皆不可讀則明示數量未知；祖先隱藏與取消保護保留。
- 實際 Python 副本中，語法合法的預設值錯誤被測試找出，小段修正後通過。fake 通過證據被拒絕；需求、測試與 revision 對應、序列化恢復及修改後失效均驗證。mock 的外部服務回傳與錯誤案例通過；缺套件、未載入副本或沒有案例均標 unavailable。
- 私有目錄寫入／讀回成功，專案原件無法讀取；父程序可連線的 loopback listener 未接受到隔離子程序連線。測試不依賴固定的 Winsock 錯誤碼，避免把 Windows 的封鎖逾時誤判為隔離失效。
- 26 個原生代理案例、9 個恢復案例、兩組 68 輪長任務、圖片／PDF、加密記憶與中斷續接通過。Python 雙程序實際重疊、相同草稿持續保存、語法／編碼、資料分析、原件保護、取消及真正 120 秒逾時通過。
- Word／Excel／PowerPoint 既有十種格式、新建三種格式、Excel 資料集與圖表、PNG 嵌入及刪除外部 PNG 後重開通過。localhost SMB 的 UNC 與映射磁碟通過；不代表公司磁碟已驗收。
- 完整 NSIS 安裝、更新、PID 交接重啟、失敗回復、保留使用者資料與解除安裝通過。Python 環境沿用，以及缺檔、同大小損壞、清單不符的完整修復均通過。

可追溯報告：[EXE](../offline/exe-verification.json)、[程式功能驗收](../offline/code-verification.json)、[Python](../offline/python-verification.json)、[並行](../offline/batch-verification.json)、[Office](../offline/office-verification.json)、[SMB](../offline/network-verification.json)、[NSIS](../offline/installer-verification.json)、[環境](../offline/environment.txt)。完整本機日誌為 `.build/release-0.8.50-complete.log`。

使用命令：

```powershell
./scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller
```

## 交付產物

| 檔案 | 版本 | Bytes | SHA256 |
| --- | --- | ---: | --- |
| `dist/LM_AI.exe` | 0.8.50.0 | 17,167,360 | `ffe8aa76da7372929ada4f23871390fab2e0850070667a870ca5c93b2490e80b` |
| `dist/LM_AI_Setup.exe` | 0.8.50.0 | 28,563,407 | `64a2cd50efeab31f8296b27f83bfba36a259398958fb9a440a727f657f8f1fc4` |

兩份更新清單均指向完整 NSIS，版本、大小與 SHA256 對應上述安裝包，並由正式 EXE 驗證簽章。Python worker／runtime 指紋與歷史 Rust 離線 ZIP 未改動；更新請使用完整 NSIS。

## 實測界線

Outlook 副本使用 COM fixture 驗證；公司信箱的四欄位實際可讀性與跨資料檔表示仍待使用者測試。Python 測試實際執行副本，可發現所列案例的錯誤，不能保證模型提出的需求與測試已完整涵蓋使用者意思。語法通過不是功能通過，mock 通過不是外部服務驗證。長任務的模型品質、速度與 tokens 仍需公司測試。
