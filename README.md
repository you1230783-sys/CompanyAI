# LM_AI — Windows 工作助理

CompanyAI 是 LM_AI 的原始碼專案。LM_AI 連接公司 AI 服務，提供日常問答、文件整理、專案副本修訂與 Classic Outlook 郵件分析。

**目前 EXE：0.8.30** · Windows 11 x64 · [版本更新紀錄](CHANGELOG.md)

## 下載與開始使用

1. [下載 LM_AI.exe](https://media.githubusercontent.com/media/you1230783-sys/CompanyAI/main/dist/LM_AI.exe)。升級時先從系統托盤選「離開」，再替換舊的 EXE。
2. 開啟程式，按「登入」，在瀏覽器核對短碼並允許授權。
3. 選擇模型後開始對話；需要讀取或修改資料夾內的文件時，先建立「專案」。

執行需要 WebView2 Runtime，不需安裝 Rust、Node 或 Python。若啟動時提示缺少 WebView2，請依 [離線安裝說明](dist/WEBVIEW2-OFFLINE.md) 處理。正式功能需要公司服務連線與桌面帳號權限。

目前 `dist/LM_AI_Setup.exe` 仍是 **0.8.15** 安裝包；本版只更新 EXE，請勿將舊安裝包或歷史離線 ZIP 視為 0.8.30。

## 可以做什麼

| 功能 | 用途 |
| --- | --- |
| 一般對話與附件 | 問答、翻譯、摘要、潤飾，支援串流與背景處理。 |
| 專案文件工作 | 選定資料夾後讀取文件、修改工作副本，成果可點擊定位。 |
| 專案技能與圖表 | 論文證據、週報增量、Excel 抽取、跨文件搜尋、批次 Office、離線圖表與快速模型摘要委派。 |
| 專案記憶 | 保存筆記、文件摘要與 PDF 轉換快取，減少反覆讀取。 |
| Outlook 助理 | 預覽與分析經確認的郵件，可授權自動補充本批郵件內文。 |
| 選字快捷鍵 | 在其他程式選取文字，帶入 LM_AI 草稿後再送出。 |
| VNC 快速連線 | 選用功能，管理機台清單並啟動已安裝的 UltraVNC Viewer。 |

專案支援 TXT、MD、Word、Excel、PowerPoint、PDF 與 MSG。各格式的可讀取／可修改範圍，請見 [使用指南](docs/USER_GUIDE.md)。

## 文件導覽

| 想了解的內容 | 文件 |
| --- | --- |
| 安裝、登入、對話與各項功能怎麼用 | [使用指南](docs/USER_GUIDE.md) |
| 每個版本改了什麼 | [版本更新紀錄](CHANGELOG.md) |
| 環境設定、編譯、測試與程式碼位置 | [開發說明](docs/DEVELOPMENT.md) |
| 後端契約、提示詞、驗證與歷史文件 | [技術文件索引](docs/README.md) |

## 開發概況

使用 Rust、Windows API 與內嵌 WebView2 介面；前端資源隨 EXE 提供，不依賴 CDN。編譯基準為 Rust 1.98.1、MSVC v142 x64 與 Windows SDK 10.0.19041.0。

維護前請閱讀 [AGENTS.md](AGENTS.md)，編譯與交付方式見 [開發說明](docs/DEVELOPMENT.md)。
