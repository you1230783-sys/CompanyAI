# 0.8.38：週報精靈、網路專案與回覆恢復

## 範圍與界線

使用者已授權實作、編譯 EXE 並推送 Git main。新增專案資料說明、下載／桌面實際位置建立、UNC／映射磁碟專案、週報三階段視窗、日期／週次提示及 Exchange／OST 線上收件匣。只發布 EXE 與簽署清單，不重建 NSIS／ZIP，不開放受控 PS。

週報準備不啟動模型；第二視窗送出後第三視窗確認，按否保留輸入。確認綁定準備代號、原專案／對話／目前登入帳號及目前任務狀態，取消不刪素材。素材路徑只由原生層建立；提示用相對路徑，資料夾名稱以 textContent 顯示。預設本週一至今天，Windows 本機日期與 ISO 週年／週次明示，使用者指定日期優先。

網路專案仍經路徑及邊界核對，禁止整個磁碟／分享根目錄、跳轉、重解析點與多重硬連結。只使用目前 Windows 權限，不接收模型提供的網路憑證；權限／伺服器查詢失敗即拒絕，不略過保護。

Outlook 沿用本機資料夾勾選、1000 封比對／50 封 AI 內文上限；新 online_inbox 不繞過資料夾祖先權限。不包含 Graph／新 Outlook；Exchange／OST 可能是快取，不保證即時伺服器資料。

無效回覆修復為兩次普通＋兩次精簡恢復，每段累計 30 次；先封存工具結果，再縮短歷史。保留 user 原文、補充、權限、額度、副本／來源版本、近期結果及查回指標。不重送未知提交，不重播已執行修改；無進展 8 次與連續工具錯誤 3 次等其他保護不變。

## 驗證狀態

2026-10-06 已完成 `scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build'`，退出碼 0。

- Rust 1.98.1、MSVC v142 14.29.30133，實際編譯器 19.29.30159（`_MSC_FULL_VER=192930159`）、x64、Windows SDK 10.0.19041.0。以空 Cargo 快取、既有 vendor／Cargo.lock／`.cargo/config.toml` 及 `--frozen` 驗證。
- 格式、Clippy、236 項 Rust 測試、release 編譯通過；WebView2 DOM 與 11 項原生控制器案例通過。
- AppContainer 檔案／網路隔離、專案成果／加密筆記／PDF、原生 HTTP 工具流程及兩種 68 輪 checkpoint 續接通過。
- 精簡恢復後沿用同一副本、已完成修改不重播；耗盡兩次一般＋兩次恢復後暫停，手動續接仍只交付一份成果。測試的舊三次錯誤 fixture 已調整為五次錯誤以驗證新門檻；修正後完整流程通過。
- 30 份約 10 MiB LOG、來源追蹤 CSV、真實 XLS／XLSX 精度及一萬筆圖表資料通過。
- Office 16.0.20430.20092：DOC／DOCX／DOCM／XLS／XLSX／XLSM／XLSB／PPT／PPTX／PPTM 讀寫、格式／公式保留、新建文件、PNG 嵌入及移除原圖後重讀通過。
- 既有 localhost SMB 分享與臨時映射磁碟實測通過：不覆寫目錄建立、TXT 讀取及成果發布、DPAPI 筆記重載、Word／Excel 建立／儲存／重讀、跳出路徑拒絕、原始檔不變。沒有建立新分享；本次臨時映射已移除。
- `dist/LM_AI.exe` 版本 0.8.38.0、13,732,864 bytes，EXE 清單已簽署並經原生驗證。清單與 EXE 的版本／大小／SHA256 核對一致；未重建 NSIS／ZIP。

EXE SHA256：`47374eec0ae24288b7794e7707b313a034fd6310a8674c5afedbd011e4055aee`。

機器紀錄：`offline/exe-verification.json`、`offline/environment.txt`、`offline/composer-verification.json`、`offline/log-verification.json`、`offline/office-verification.json`、`offline/network-verification.json`。

新增回歸包括 ISO 跨年週次、路徑正規化／拒絕、週報未確認不啟動／跨對話拒絕／重複提交拒絕、前端按否保留內容／名稱不執行 HTML、Exchange 收件匣 COM fixture、上下文恢復保留指示／副本及不重播修改。

本機 SMB 測試不等於公司 F 槽／UNC、重新導向 Known Folder、公司加密系統的驗收；記憶體 COM fixture 不等於真實公司 Outlook。GLM-5.3-Flash 選材、日期要求理解及最終週報格式／品質待使用者實測。DOM／原生控制器檢查不等於外觀截圖驗收；模擬期限不等於實際連跑 24 小時。
