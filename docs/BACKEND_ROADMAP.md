# 後端修改總表與後續規劃

依使用者分享對話及後續確認整理。0.4.0 將通知與 Classic Outlook 納入本次；其餘建議未實作。

| 項目 | 0.4.0 狀態 | 契約 |
| --- | --- | --- |
| 瀏覽器登入、個人 Token、固定路由 | 已實作 | WEB_INTEGRATION.md |
| 模型 alias、Chat Completions、stream=false | 已實作 | WEB_INTEGRATION.md |
| latest／minimum 版本門檻、連線失敗暫用 | 已實作 | WEB_INTEGRATION.md |
| 選取文字＋全域快捷鍵、確認送出 | 已實作 | README.md |
| Markdown、公式、程式碼、緊湊介面與本機對話 | 已實作 | README.md |
| 網站通知、REST 補查、WebSocket、已讀 | 已實作 | NOTIFICATIONS_AND_OUTLOOK.md |
| Classic Outlook 單封唯讀預覽、確認正文／分析 | 已實作，待公司 Office 實機驗收 | NOTIFICATIONS_AND_OUTLOOK.md |
| 長任務狀態／結果、附件、OCR、RAG | 尚未實作 | 後續另訂 |
| UNC 知識庫、skills、AI 工具呼叫 | 僅討論，未實作 | 後續另訂 |
| 寄信、回填、刪信、任務外部操作 | 尚未實作 | 需另訂操作與確認流程 |

網站這次請先依 [NOTIFICATIONS_AND_OUTLOOK.md](NOTIFICATIONS_AND_OUTLOOK.md) 建立事件資料、REST 與已讀，再加入 WebSocket。Outlook 透過桌面使用者的 COM 物件讀取，網頁不直接讀公司電腦。
原有登入／模型／版本／聊天保持 [WEB_INTEGRATION.md](WEB_INTEGRATION.md) 契約。通知 API 尚未部署時一般聊天可繼續使用，通知會提示並自動重試。

未來知識庫可由網頁提供核准 UNC 根目錄，桌面以目前 Windows 使用者權限讀取，AI 只取得知識庫 ID 與相對路徑。需要工具白名單、檔案範圍檢查、連結處理、讀取上限及受控工具往返，不能只用字串替換根路徑。使用者指定這部分先討論，本版沒有相關網路呼叫或檔案工具。

後續順序為公司驗證目前功能，再獨立設計唯讀知識查詢，最後才考慮附件、長工作與會改變外部資料的操作。不要以本文件的未來建議視為已授權實作所有功能。
