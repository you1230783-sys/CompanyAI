# 技術文件索引

[專案首頁](../README.md) · [使用指南](USER_GUIDE.md) · [版本更新](../CHANGELOG.md) · [開發說明](DEVELOPMENT.md)

一般操作請先閱讀使用指南。以下為開發、串接及問題追蹤文件；以版本命名的文件記錄當時契約，後續版本可能覆蓋部分規則，不應直接視為目前操作方式。

## 目前版本

- [0.8.25 工具契約](DESKTOP_0_8_25_CONTRACT.md)：桌面文字 tools／tool_calls、舊格式相容與執行界線。
- [0.8.25 驗證紀錄](VALIDATION_0_8_25.md)：編譯器、測試、EXE 雜湊與未測範圍。
- [0.8.25 完整請求範例](PROJECT_REQUEST_0_8_25_EXAMPLE.json)：包含完整 system，資料為虛構範例。

## 專案文件與模型流程

| 文件 | 主題 |
| --- | --- |
| [0.8.24 契約](DESKTOP_0_8_24_CONTRACT.md) | 長文件可選筆記與閱讀計數。 |
| [0.8.23 契約](DESKTOP_0_8_23_CONTRACT.md) | .lmai、文件摘要、快取及上下文選取。 |
| [0.8.22 契約](DESKTOP_0_8_22_CONTRACT.md) | 有限續接、格式修復與任務上限；當時的強制筆記已由 0.8.24 取代。 |
| [0.8.21 契約](DESKTOP_0_8_21_CONTRACT.md) | 專案 PDF 伺服器轉換路由。 |
| [0.8.18 契約](DESKTOP_0_8_18_CONTRACT.md)、[0.8.19 契約](DESKTOP_0_8_19_CONTRACT.md) | Office 副本、舊格式與 MSG；當時 PDF 本機流程已由後續版本調整。 |
| [0.8.16 契約](DESKTOP_0_8_16_CONTRACT.md)、[0.8.17 契約](DESKTOP_0_8_17_CONTRACT.md) | 專案授權、TXT／MD、介面、歷程與重試。 |
| [0.8.24 提示詞快照](PROJECT_PROMPT_REFERENCE.md) | 前一版提示詞及訊息組裝說明，保留作比較。 |

## 後端、部署與功能細節

| 文件 | 主題 |
| --- | --- |
| [網站串接契約](WEB_INTEGRATION.md) | 後端入口與版本補充；先讀文件頂部的最新變更。 |
| [EXE／NSIS 更新契約](UPDATE_0_8_1_CONTRACT.md) | 下載清單及簽章；選版優先順序與一次確認流程以 0.8.16 契約為準。 |
| [結構化回答契約](STRUCTURED_REPLY_CONTRACT.md) | 正文、重點、來源、限制與引用。 |
| [VNC 詳細說明](VNC_QUICK_CONNECT.md) | 機台管理、網站同步、Viewer 與設定格式。 |
| [Outlook 查詢說明](OUTLOOK_SEARCH.md) | 郵件查詢範圍與資料欄位，配合後續版本契約閱讀。 |
| [離線交付與 Git](OFFLINE_AND_GIT.md) | 工具鏈、vendor、封裝與校驗；現存 ZIP 為歷史版本。 |
| [WebView2 離線安裝](../dist/WEBVIEW2-OFFLINE.md) | Runtime 安裝與部署。 |

## 歷史資料

各版本的 `DESKTOP_*_CONTRACT.md`、`VALIDATION_*.md` 與 `PENDING_*.md` 保留作決策及驗收追蹤；PENDING 文件不一定代表仍未發行，請查看其最新註記與對應版本。版本摘要集中於 [CHANGELOG](../CHANGELOG.md)。

0.8.0 以前只在更新紀錄保留簡短摘要。需要追查原始協定時，可看 [0.7 契約](DESKTOP_0_7_CONTRACT.md)、[0.6 契約](DESKTOP_0_6_CONTRACT.md)、[0.5 契約](DESKTOP_0_5_CONTRACT.md)、[早期通知／Outlook 契約](NOTIFICATIONS_AND_OUTLOOK.md) 與 [快捷鍵相容性說明](HOTKEY_0_4_1.md)。

[後端規劃](BACKEND_ROADMAP.md) 與 [早期介面說明](UI_AND_STREAMING.md) 是歷史設計資料；目前是否已實作請核對最新契約及程式碼。
