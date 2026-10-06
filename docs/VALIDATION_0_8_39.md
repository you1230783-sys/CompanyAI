# 0.8.39：Outlook 專案入口與圖片試驗

## 交付範圍

使用者授權實作、進版、編譯 EXE 與部署既有 Git main。專案新增 Outlook 助理及圖片辨識試驗按鈕；原週報流程保留。Outlook 新入口共用專案郵件技能、資料夾同意／勾選、1000 封本機比對與 50 封 AI 內文額度。舊獨立 Outlook 預覽與 MSG 功能仍保留，未宣稱所有入口已完全合併。

圖片限定專案內 JPG／JPEG／PNG、單張 1 MiB、8192×8192 以內且不超過 1600 萬像素。沿用路徑、重解析點、硬連結及內部目錄保護，有限讀取後計算 SHA256。不讀 Outlook 附件／內嵌圖、不處理掃描 PDF、不新增影像解碼或外部執行環境。

`image-read` 按需啟用 `analyze_image(path,focus)`；獨立子請求使用目前模型、既有公司 API 與身分驗證，user.content 為 text＋image_url data URL 陣列，tools=[]，結果只回文字及來源。每次任務最多 20 次不同辨識要求；同任務、圖片版本、焦點及身分相同時重用結果。未知請求 DPAPI 保存，續接僅查原 ID；已知終態清除圖片快照，保留結果。JPEG 只檢查容器／尺寸，像素是否可解碼由模型端處理。

## 驗證狀態

2026-10-06 完成 `scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build'`，退出碼 0。

- Rust 1.98.1、MSVC v142 14.29.30133，實際編譯器 19.29.30159（`_MSC_FULL_VER=192930159`）、x64、Windows SDK 10.0.19041.0。使用空 Cargo 快取、既有 vendor／Cargo.lock／`.cargo/config.toml` 與 `--frozen`，未新增依賴。
- 格式、Clippy、239 項 Rust 測試、release 編譯全部通過；WebView2 DOM 與 13 項原生控制器案例通過。
- 新增原生 Outlook／圖片快速入口驗證：準備不啟動模型、跨對話及模型變更拒絕、圖片越界拒絕、保留使用者補充、載入對應技能、重複提交拒絕。前端驗證用途提示、過期回覆忽略、圖片路徑不執行 HTML、送出失敗保留輸入。
- 五種圖片 HTTP 整合案例通過：JPG、PNG、POST 回覆遺失後續接、越界來源、過大圖片。測試伺服器反解 Base64 核對完整來源 bytes；前兩種及續接案例各有 4 次父請求、僅 1 次圖片子請求，重複要求命中快取，無效來源的圖片 POST 為 0。主歷史沒有 data URL，圖片子請求為目前模型、無工具，回覆附來源 SHA256。
- AppContainer 檔案／網路隔離、30 份約 10 MiB LOG、CSV、專案記憶、PDF、工具修復及兩種 68 輪 checkpoint 續接全部通過。模擬程序中斷的 panic 是預期測試事件，恢復案例通過且未重播已完成操作。
- 實際 Office 的 DOC／DOCX／DOCM／XLS／XLSX／XLSM／XLSB／PPT／PPTX／PPTM 讀寫、新建文件、XLS／XLSX 精度／CSV／一萬筆圖表，以及 PNG 嵌入、移除原圖後重讀通過。
- 既有 localhost SMB UNC 與臨時映射磁碟的資料夾建立、TXT 成果發布、DPAPI 筆記、Word／Excel 儲存重讀、越界拒絕及原始檔不變均通過；未新增分享，臨時映射已移除。
- `dist/LM_AI.exe` 版本 **0.8.39.0**，**13,895,168 bytes**。EXE 清單已簽署並由原生驗證；版本、大小、SHA256 與發行檔一致。

EXE SHA256：`b61dbf62df40bdeec05ab9d3121abb1fb47fdc007409adddd408317cbd34bc13`。

機器紀錄：`offline/exe-verification.json`、`offline/environment.txt`、`offline/composer-verification.json`、`offline/log-verification.json`、`offline/office-verification.json`、`offline/network-verification.json`。

## 實測限制

使用者已獨立驗證模型伺服器 image_url，但本機無公司模型登入環境。Loopback HTTP 使用固定回覆，驗證的是 JSON 結構、原始圖片 bytes、子請求控制、快取與續接，不是辨識品質。公司轉發 API 是否放行 content 陣列／data URL 及模型辨識能力仍需使用者用 EXE 驗收；不改模型直連、不關閉 TLS 驗證。

本機 SMB／Office 回歸不等於公司 F 槽／UNC／加密系統驗收；COM fixture 不等於真實公司 Outlook 信箱。UI 原生及 DOM 測試不代表已取得外觀截圖。只發布 EXE／簽署清單，不重建 NSIS／ZIP。
