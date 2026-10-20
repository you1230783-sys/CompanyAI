# 技術文件索引

目前 EXE／NSIS：[0.8.75 驗證紀錄](VALIDATION_0_8_75.md)，55 個假資料互動教學與獨立預覽。實際建置、瀏覽器回歸及封裝範圍分開記錄；歷史版本的驗證不當作本版重測。

前置設計：[參考線文字排版與預設配色](PENDING_CHART_LAYOUT.md)，已併入0.8.48；該文件保留當時只修改來源的驗證界線。

[專案首頁](../README.md) · [使用指南](USER_GUIDE.md) · [版本更新](../CHANGELOG.md) · [開發說明](DEVELOPMENT.md)

一般操作請先閱讀使用指南。以下為開發、串接及問題追蹤文件；以版本命名的文件記錄當時契約，後續版本可能覆蓋部分規則，不應直接視為目前操作方式。

## 目前版本

- [0.8.75 互動教學](VALIDATION_0_8_75.md) · [55 個教學清單](TUTORIAL_COVERAGE.md) · [預覽與維護](PROJECT_TUTORIAL_PREVIEW.md)。

- [0.8.74 閱讀筆記與交付核對](VALIDATION_0_8_74.md) · [八項實作紀錄與公開專案](IMPLEMENTATION_2026_10_20.md)。

- [0.8.73 介面偏好與專案修正](VALIDATION_0_8_73.md) · [本版修改清單](NEXT_VERSION_NOTES.txt)。

- [0.8.72 Mermaid、CAD 與確認通知](VALIDATION_0_8_72.md) · [Mermaid 操作](MERMAID.md) · [CAD 能力](CAD_CAPABILITIES.md) · [公司測試提示詞](COMPANY_TEST_PROMPTS_0_8_72.txt)。

- [0.8.71 CAD 註記、版本與額度續接](VALIDATION_0_8_71.md)。

- [0.8.70 分段計畫與真 Strata 雙版驗證](VALIDATION_0_8_70.md) · [雙版 DWG ZIP](../examples/cad/cad-segmented-0.8.70.zip)。
- [本機模型驗證流程與分級](LOCAL_MODEL_VALIDATION.md) · [多類使用者 CAD 圖型矩陣](VALIDATION_CAD_USER_SCENARIOS.md)。

- [0.8.69 CAD 批次讀取、Luna 與實機驗證](VALIDATION_0_8_69.md)。
- [CAD 本機快照、候選查詢與局部預覽](CAD_SNAPSHOT_QUERY.md)。
- [0.8.68 發行驗證](VALIDATION_0_8_68.md) 與 [當時的 Luna 實測／批次原型](VALIDATION_LUNA_CAD_INDEX.md)。

### 先前版本紀錄

- [0.8.54 程式碼唯讀與浩辰 CAD 測試](VALIDATION_0_8_54.md)。
- [獨立 CAD 工具操作](../tools/gstarcad-probe/README.md)。

- [0.8.53 聊天任務停止追蹤與介面整理](VALIDATION_0_8_53.md)：413 原始錯誤、三次相同 GET 錯誤、分析摺疊與圖中文字雙擊。
- [0.8.52 分析成果與 Outlook 相容性](VALIDATION_0_8_52.md)：分析完成摘要、LOG 時間表、內容比較、方法重用與來源預覽。

- [0.8.48 圖中文字、AI編輯與Python試用](VALIDATION_0_8_48.md)：文字樣式、排版與配色、本機時間、.py副本與語法檢查。

- [0.8.47 郵件成果與圖表診斷](VALIDATION_0_8_47.md)：來源摘要、草稿、工作階段、量測軸、參考線、頁籤與本機日期。
- [錯誤代碼與匯出](ERROR_CODES.md)：Error Code、可能原因與獨立診斷檔。
- [0.8.46 工作筆記與上下文](VALIDATION_0_8_46.md)：動態技能、最近工具往返、DEBUG、非阻塞偏好與一般對話表格。

- [0.8.45 長任務恢復與分析依據](VALIDATION_0_8_45.md)：本機預設專案、映射磁碟、上游延後恢復、Outlook實際進度、證據及方法記憶。

- [0.8.44 圖表編輯與工具重試](VALIDATION_0_8_44.md)：對話框按鈕底色、三頁編輯器與範例、數值軸自動取整、空白參考線、X／Y 提示及獨立5／10次失敗上限。
- [0.8.43 圖表座標、Python LOG 與安裝沿用](VALIDATION_0_8_43.md)：X／Y平移及重新編號、五點示意、Big5 LOG、欄位錯誤診斷與專案圖片一般支援。
- [Python 執行環境](PYTHON_RUNTIME.md)：固定套件、資料橋接、編碼、NSIS沿用與離線重建。

### 近期版本紀錄

- [0.8.42 離線 Python 與完整 NSIS](VALIDATION_0_8_42.md)。
- [0.8.41 圖片關聯與閱讀位置](VALIDATION_0_8_41.md)。
- [0.8.40 Excel 規劃與圖表編輯](VALIDATION_0_8_40.md)。
- [0.8.39 Outlook 專案入口與圖片試驗](VALIDATION_0_8_39.md)。
- [0.8.38 週報精靈、網路專案與回覆恢復](VALIDATION_0_8_38.md)。
- [0.8.36 本地 CSV 與主動上下文整理](VALIDATION_0_8_36.md)：批次擷取、首尾預覽、本地畫圖、補充更正與加密交接。
- [0.8.35 LOG 定位與主輸入框](VALIDATION_0_8_35.md)：近似時間、三種傳送模式與進度筆記。
- [0.8.34 大型 LOG 與長任務](VALIDATION_0_8_34.md)：Outlook 同意、24 小時與 checkpoint。

- [0.8.32 Office 圖片與進階診斷](VALIDATION_0_8_32.md)：PNG 嵌入、工具可用條件與網站上游正規化配合。
- [0.8.31 圖表與本機診斷驗證](VALIDATION_0_8_31.md)：10000 筆圖表、PNG 匯出、工具錯誤及逐輪執行紀錄。
- [0.8.30 Schema 修正及驗證](VALIDATION_0_8_30.md)：修正 Office 表格與圖表多型別造成第一輪 UNSUPPORTED_SCHEMA。
- [原生工具 desktop-agent-v1 契約](DESKTOP_AGENT_V1_CONTRACT.md)：獨立專案代理路由、原生訊息、能力協商、持久 JSON 與雙方驗收。
- [桌面端實作與驗證狀態](VALIDATION_0_8_29.md)：0.8.29 EXE 已接入原生工具與 Excel 選欄，實際網站與模型待聯合驗收。

- [0.8.28 契約](DESKTOP_0_8_28_CONTRACT.md)：技能、圖表、跨文件搜尋、批次 Office 與快速摘要委派。
- [0.8.28 驗證紀錄](VALIDATION_0_8_28.md)。

- [0.8.27 契約](DESKTOP_0_8_27_CONTRACT.md)：兩小時時限、異常暫停與原請求查回。
- [0.8.27 驗證紀錄](VALIDATION_0_8_27.md)。

- [0.8.26 契約](DESKTOP_0_8_26_CONTRACT.md)：Office 新建與格式、上限暫停續接、HTTP 階段診斷。
- [0.8.26 驗證紀錄](VALIDATION_0_8_26.md)。

### 前版協定參考

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

- [0.8.33 圖表資料決策與按需工具驗證](VALIDATION_0_8_33.md)

- [0.8.34 大型 LOG、Outlook 與長任務驗證](VALIDATION_0_8_34.md)
- [0.8.35 進度筆記、LOG 定位與主輸入框驗證](VALIDATION_0_8_35.md)

- [0.8.37 Outlook資料夾／本機前文比對／閱讀額度](VALIDATION_0_8_37.md)
