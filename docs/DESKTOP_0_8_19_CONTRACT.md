# 0.8.19 文件工具契約

本版覆蓋 0.8.17 的包裝 JSON 重試規則及 0.8.18 的格式清單；其餘授權、原件保護、版本比對與成果驗證不變。

## 說明與工具往返

每則模型回覆最多 64 KB，允許純 JSON，或文字／Markdown fence 包住一個完整工具物件。以 JSON parser 取得物件邊界，字串中的括號不當作結束；必須反序列化為既有嚴格 Decision/Tool 結構，再交由 broker 檢查工作區、操作代號、檔案與版本。未知工具、額外物件、陣列包裝或歧義停止。不完整 JSON 最多要求兩次修正，修正前不執行。

非 JSON 說明以 `AI 說明：` 保存在既有活動記錄，單項最多 2048 字、最多 120 項。前端用 textContent 直接顯示說明，同時保留可摺疊工具歷程；說明不表示工具已成功。完成與詢問仍使用純 JSON。既有 operation_id 重播與成果核對維持。

## 成果定位

相對路徑經元件驗證後重建為 Windows 分隔符號。僅 `_AI_Output` 檔案可定位，逐層固定目錄與檔案 handle，拒絕越界、reparse point、硬連結與目录。先用 Shell PIDL 定位，若公司 Shell 回報類別／參數錯誤，改以 GetWindowsDirectoryW 取得固定 explorer.exe，傳 `/select,"完整路徑"`。不經 CMD／PowerShell／檔案關聯，不開啟文件本身。

## PDF／MSG

- PDF：pdf-extract 0.12.1 及鎖定依賴內嵌，僅在既有 AppContainer 解析 broker 傳入的 bytes。固定 `PDF\0` 管線訊息，20 MB 輸入，200 頁，200 KB 文字；原有 JSON 操作仍限 800 KB。子程序保持無外部文件／網路權限、128 MB Job 記憶體限制，回覆限時 15 秒，逾時／取消終止 Job，避免遲到回覆套到下一次操作。
- 輸出帶頁碼，空白頁標示未擷取文字；全部無文字則失敗。文字順序、表格、特殊字型及掃描內容不保證還原，不做 OCR，不執行 PDF JavaScript／附件／外部程式。不把解碼當解密，解析失敗可匯入核准閱讀器文字。
- MSG：上限 50 MB，由已開啟且完成設定的 Classic Outlook `GetNamespace("MAPI").OpenSharedItem` 讀取指定檔案。只接受 MailItem（Class 43），只取主旨、寄件者、收件者、副本、寄件時間、純文字正文；結果限 200 KB。不枚舉信箱、不送信、不讀附件、不匯入 Store；結束 Close(olDiscard)，不 Quit 使用者 Outlook。
- MSG／Office COM 位於受信任 broker，**不在 AppContainer 中**；若 COM 等待原生視窗，取消需等呼叫返回。本機 MSG 尚未實測成功，不宣稱公司加密支援已驗收。
- `read_file`／`find_text` 支援新增格式；`create_working_copy(source=PDF或MSG,name=*.txt)` 建立 UTF-8 BOM 文字副本，後續 edit_text/save_copy 不變。不可輸出修改後 PDF／MSG。TXT/Office/PDF/MSG 內容禁止流入 MD。匯入文字選檔新增 PDF／MSG，快照維持 DPAPI，需手動更新。

## Office

| 格式 | 另存常數 | 修改範圍 |
|---|---|---|
| DOC / DOCX / DOCM | 0 / 12 / 13 | 既有正文段落，包含既有表格內段落 |
| XLS / XLSX / XLSM / XLSB | 56 / 51 / 52 / 50 | UsedRange 文字與數字，公式與合併格唯讀 |
| PPT / PPTX / PPTM | 1 / 24 / 25 | 一般文字框與標題 |

Office 原件唯讀；保留檔案格式與來源指紋，副本修改後重新開啟核對。未增加建立表格／列欄／合併儲存格、任意 COM 或巨集工具。VBA 以 AutomationSecurity=ForceDisable 開啟，不自動信任文件；Excel 4.0 XLM 不受此設定全面控制，仍由公司 Trust Center 政策決定，**不自動接受安全提示，也不宣稱可在不安全的 Office 政策下執行任意巨集文件**。測試 fixture 為巨集容器格式但不含真實 VBA/XLM，不能據此宣稱所有巨集文件均已驗證。

## 交付

版本 0.8.19，MSVC v142 x64／空 Cargo 快取／--frozen。EXE 與其簽署清單成對發布；NSIS 及歷史 ZIP 不重建。新 PDF 依賴已同步 Cargo.lock、vendor，.cargo/config.toml 仍指定本專案 vendor。
