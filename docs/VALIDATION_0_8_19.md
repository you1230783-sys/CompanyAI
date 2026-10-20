# 0.8.19 驗證紀錄

2026-09-29 執行 `scripts/Build.ps1 -EmptyCargoCache -TestOffice` 成功；只交付 EXE，不重製 NSIS 或歷史 ZIP。下列本機驗證不代表公司加密環境已驗收。

- MSVC v142 14.29.30133、實際 `_MSC_FULL_VER=192930159` x64、Rust 1.98.1、SDK 10.0.19041.0。
- Cargo.lock、vendor、.cargo/config.toml 齊備；空 Cargo 快取、`--frozen`、fmt、Clippy `-D warnings`、119 項測試、release、真正 WebView2 自檢均通過。
- AppContainer 外部檔案／網路拒絕、Unicode 副本、操作重播、來源保護、成果重新讀回、越界與硬連結拒絕通過。PDF 使用相同隔離程序。
- 三組真正 HTTP 工具往返：純 JSON 成功；說明＋完整編輯 JSON 不重送且只執行一次；不完整 JSON 最多修正兩次後停止。活動記錄保留說明。
- 前端 23 項回歸檢查通過，含執行中說明直接顯示、完成後保留、HTML 字串不執行。已檢視前端活動畫面截圖；不是 Office 文件逐頁視覺驗收。
- 原生 Explorer 實測正斜線、中文、空白檔名均在正確目錄被選取。另以未初始化 Shell 的探針實際取得 `0x80070057`，驗證備援 Explorer 同樣選取正確檔案；越界與非成果路徑拒絕。只關閉測試新建視窗。
- PDF 經真實 AppContainer 擷取 ASCII 與 Word 輸出的繁體中文內容；頁碼、2 MB 傳輸、全頁無文字／損壞檔案拒絕、TXT 成果讀回與原檔不變通過。未支援 OCR、圖片、原始 PDF 修改，不保證所有字型及版面順序。
- Office 16.0.20326.20158 實際建立並處理 DOC/DOCX/DOCM、XLS/XLSX/XLSM/XLSB、PPT/PPTX/PPTM；逐一修訂、另存、重新開啟、第二版本 `_2`、重送不重複存檔與原件 bytes 不變通過。
- 四種 Excel 格式均驗證數字修改、公式拒絕、`=1+1` 按文字儲存。另由 Office 重開 DOCX/XLSX/PPTX 核對粗體及未修改段落／公式／文字框。
- 巨集格式 fixture 不含真實 VBA/XLM。未測真實巨集、公司 Trust Center、受保護檢視、第三方加入項與複雜排版。
- **MSG 實機驗收未完成**：本機 Classic Outlook 停在首次設定，COM 回報 `RPC_E_CALL_REJECTED`，未更改帳號或安全設定。MSG 正式工具改為只連接已開啟且完成設定的 Classic Outlook，並提供手動匯入文字；不宣稱已成功解析公司 MSG。可在有設定的電腦執行 `scripts/Test-Msg.ps1 -Exe <EXE> -Probe <msg_smoke.exe>`。
- EXE：8,180,224 bytes；SHA256 `6f01c77ab69f8583043aaeac9d8e98bd1db4e66ac42f3050d5ed9626f7f7f702`，與簽署 EXE 更新清單一致。

機器記錄：`offline/exe-verification.json`、`offline/office-verification.json`、`offline/document-verification.json`。NSIS 與其清單仍為 0.8.15；Git 推送不代表內網網站已部署。

公司待測：加密 PDF／MSG 直接讀取、TXT 輸出加密後讀回、原有加密 Office 流程、實際 GLM／Gemma 工具回覆。Office／Outlook COM 仍可能等待原生視窗，取消需等目前呼叫返回。
