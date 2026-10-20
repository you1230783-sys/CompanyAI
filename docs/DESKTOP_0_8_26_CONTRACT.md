# 0.8.26 Office 初版與手動續接

## Office 能力

沿用安裝於電腦的 Word、Excel、PowerPoint 與固定 Rust COM 呼叫，不使用模型腳本。Office 程序屬受信任桌面端，不在文字 AppContainer 中。巨集與外部連結設定沿用既有禁用流程。

- `create_working_copy(source:null,name)` 新建 TXT、DOCX、XLSX、PPTX；Office 新建不要求來源文件。
- 既有 DOC/DOCX/DOCM、XLS/XLSX/XLSM/XLSB、PPT/PPTX/PPTM 副本保持來源格式。
- `edit_office` 保留既有區塊原文核對；Word/PPT 只替換真正變動的文字範圍，UTF-16 定位不切斷代理對。
- `office_action` 接收 copy_id、revision、operation。固定種類：format、word_paragraph、word_table、excel_sheet、excel_write、ppt_slide。
- Word 在正文指定段落之前或文件結尾新增段落、簡單矩形表格；不在表格內插入巢狀結構。文字格式以整段為單位。
- Excel 範圍限定 A1 矩形，前 1000 列／100 欄、單次最多 2000 格；新工作表至多 50 張。文字以字面值保存，不寫入公式、不覆寫公式或合併格。數字格式使用固定預設名稱。
- PPT 新增 title/content/two_column 版面，格式以一般文字框為單位。最多 200 張；不操作群組、SmartArt、表格、圖表、圖片或備忘稿，不保證文字不溢出。
- 常用屬性：字型、Word 東亞字型、字級、粗／斜體、色彩、對齊、段距／倍數行距、項目符號；Excel 另有底色、框線、換行、欄寬、數字格式。未指定屬性保持原值，新段落／新表格先套用正文格式。

工作版本保存固定操作清單，每次在新開的唯讀來源或空白文件上重建、執行並擷取快照，成功才接受新版本。失敗操作關閉不儲存，不污染已接受的副本。單文件最多 200 次修改；反覆重建較耗時，初版適合小型文件與批次範圍操作。

快照包含 scope、structure、formats、blocks。blocks.format 是 formats 的索引，相同格式共用，混合屬性為 null；Office 浮點格式數值正規化至千分之一點。來源 bytes fingerprint 與快照均需相符，結構改變後回傳新 revision／區塊資訊。整份快照仍以 200 KB 為上限。

`save_copy` 先重建至本次獨立 staging，經 create_new handle 發布至 `_AI_Output/YYYYMMDD_HHMMSS/可讀檔名`，Office 重新開啟核對結構、文字及已追蹤格式。新檔預設 Open XML；原件仍無寫入入口。公式、巨集、外部連線及複雜物件未因新增排版而開放。

## 上限暫停與繼續

每段最多 60 次工具／80 次模型回覆，剩餘四次時提示模型在 progress_note 整理已完成、來源版本、未完成事項與下一步。程式不依賴筆記才能暫存：正常達上限即保存 broker 工作副本（包括未儲存內容）、操作 ID 與參數／結果去重表、輸出紀錄及 Progress，使用 Windows DPAPI 保護。

暫停檔位於應用資料 `project-runs/<run_id>.resume.dpapi`，不存 Token、不存程序／COM 物件。只有正常暫停顯示「繼續」；任務執行器返回並釋放 AppContainer，暫停期間沒有常駐工作程序。可關閉應用後回到同一對話續接。

續接必須由使用者點擊，核對最新訊息、同一專案 ID／根目錄／對話與後端 principal_id。Office 來源 fingerprint 及已儲存最新成果需相符；檢查失敗不消耗暫存點。通過後消耗該暫存點，防止重複點擊或當機後自動重播不明操作。網路結果不明、取消及執行中當機均不屬此續接入口。

每段重新計數，舊的去重表保留；相同操作 ID、相同參數只回原結果，不重做修改或發布。新操作必須使用新 ID。模型上下文只附最近兩筆結果、摘要、閱讀與副本狀態，完整舊工具結果保存在操作簿，透過 `read_work_log(offset)` 每次 6000 字查回；按 ID 排序而非時間排序。此工具查閱本身不遞迴放入回傳的操作簿。

連續失敗、無進展、30 分鐘與文字預算等既有界線仍有效；這些原因不會自動擴權或無限重跑。完成條件仍由模型判斷，檔案真實性及版本由程式核對。

## 第 20 輪與 404 診斷

專案不使用一般對話的 20 輪限制。每輪 POST 失敗後查回同一 request_id，最多連續三次 GET 失敗才停止；不換 ID、不重送 POST。錯誤保留最初提交階段、HTTP 狀態及最後查詢路由，避免把後端最初的拒絕誤報成只有查詢 404。

讀取 error.message 或統一格式 message，遮蔽 Token、限制長度；404 提示為指定資源或路由不存在，不直接斷定整個服務未部署。正式那次原因仍需 request_id 對照公司伺服器日誌。後端應確認專案 messages 大於 40 則時沒有套用一般聊天限制。

網頁 API 不變，仍 `skills:false` 與 messages 文字 tools/tool_calls；不新增原生 tools HTTP 欄位。只交付 EXE，不製作 NSIS 或離線 ZIP。
