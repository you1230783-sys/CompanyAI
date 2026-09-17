# LM_AI 0.4.0：網站通知與 Classic Outlook 契約

本文件將先前 BACKEND_ROADMAP 的通知／Outlook 草案落實為 0.4.0 契約。其餘附件、RAG、UNC 知識庫、工具呼叫、寄信、任務自動化均未實作。

## 網頁端這次需要做什麼

1. 保留既有登入、版本、模型與 Chat Completions 路由。
2. 建立依使用者／群組授權的事件資料、接收對象、已讀狀態與可補查的游標。
3. 實作下列 GET、POST 路由及 WebSocket；事件必須先存入資料庫，再發出喚醒訊號。
4. 確認反向代理允許 WebSocket Upgrade，並轉送 Authorization Header。
5. Outlook 不需新增郵件讀取 API；桌面透過使用者的 Classic Outlook 讀取，經確認後沿用聊天 API 分析。

所有通知請求使用 `Authorization: Bearer <個人 access_token>` 與 `X-Client-Version: 0.4.0`。沿用既有個人 Token；本版登入申請 scope 仍為 `chat:write`，後端須將桌面通知存取納入此授權，或另行協調 scope 升版。不要在 URL 傳 Token。

## 事件補查

```http
GET /lm_server/api/desktop/events?after=c_000123 HTTP/1.1
Authorization: Bearer <個人 access_token>
X-Client-Version: 0.4.0
```

首次查詢省略 `after`，回傳此使用者仍有效的事件。HTTP 200：

```json
{
  "events": [
    {
      "id": "evt_000124",
      "type": "notice",
      "title": "系統公告",
      "summary": "新功能已開放使用。",
      "created_at": "2026-09-17T02:00:00Z",
      "expires_at": "2026-09-24T02:00:00Z",
      "resource_id": null,
      "read_at": null
    }
  ],
  "next_cursor": "c_000124",
  "has_more": false
}
```

- `id` 為 1–128 字元 ASCII 英數、點、底線或減號；全域或帳號內唯一且穩定。
- `title` 必填，最多 150 字元；`summary` 最多 2,000 字元，省略則空白；`type` 最多 100 bytes。
- `created_at` 必填，時間皆為帶時區的 RFC 3339。`expires_at`、`resource_id`、`read_at` 可省略或為 null。
- 每頁最多 100 筆，整個 HTTP 回應不超過 1 MB。`has_more` 省略視為 false。
- 游標是後端不透明字串，最多 2,048 bytes；不得使用客戶端時間作為唯一游標。App 會 URL encode。
- `has_more=true` 必須提供不同於上一頁的非 null `next_cursor`。最後一頁也應提供可供下次增量查詢的游標；沒有變動可回原游標與空 events。
- 游標表示「事件變更串流」的位置；跨裝置標記已讀也應產生可補查的變更，回傳同 id 的完整事件與新 read_at。
- 過期游標請回傳仍有效的完整快照及新的游標，讓 App 合併恢復。游標長期不能使用但僅回錯誤，App 會反覆重試舊游標。
- 以 id 合併，重複傳送不重複顯示。已讀不回退未讀；0.4.0 不支援「改回未讀」或刪除事件的 tombstone。
- 撤下事件可用同 id 更新 expires_at 為過去時間；不要僅從資料庫刪除而不產生變更，否則離線快取無從得知。
- 伺服器必須每次依 Token 與接收對象授權，不相信游標、事件 id 或前端已知清單就是授權。

App 登入、WebSocket 連線／喚醒、每 60 秒及手動重新整理時補查。成功加密保存一頁後才採用游標，避免寫入失敗漏事件。本機最多保留 500 筆有效通知，快取綁定 Token 指紋；换 Token 會重新補查，由後端保存的已讀狀態恢復。

## 即時喚醒

連線位置為 `ws://lp2-en-server/lm_server/api/desktop/events/ws`，使用標準 HTTP Upgrade；未來固定主機改為 HTTPS 時對應 WSS。
伺服器驗證 Header 後回 101。文字訊息範例：

```json
{"type":"events_available"}
```

App 收到合法 JSON 的非 ping 訊息後補查 REST，不直接信任 WebSocket payload 作為通知內容，也不執行其中的連結或指令。
單則訊息上限 64 KB；二進位訊息不支援。可使用標準 WebSocket ping/pong 保活，或 `{"type":"ping"}` 應用層保活；後者 App 忽略且不回應，後端勿要求文字 pong。
斷線以 5、10、20、40、60 秒退避重連，REST 仍定期補查。登出取消連線；後端也應在 Token 到期／撤銷時關閉連線。

## 標記已讀

```http
POST /lm_server/api/desktop/events/evt_000124/read HTTP/1.1
Authorization: Bearer <個人 access_token>
Content-Type: application/json

{}
```

成功可回 204，或 200 加任意 JSON。操作需冪等；使用者不可藉猜 id 標記他人的事件。App 成功後更新本機 read_at，再由後續補查取得後端時間。
未授權回 401；無權限回 403；缺少事件回 404。錯誤不把通知當成已讀，也不影響一般聊天。

## 桌面通知行為

通知中心顯示標題、摘要、時間及已讀按鈕。Windows 系統列提示只顯示「收到 N 則新通知」，避免鎖定畫面顯示郵件正文；不搶焦點，點擊才開通知中心。
Windows 通知設定／勿擾模式可能抑制彈出提示，通知中心仍保留資料。設定頁可關閉彈出提示。
程式開啟或最小化時接收；按關閉即退出，沒有開機常駐、背景服務或退出後推播。本版保存 resource_id，但不自動開工作詳情，沒有 `/tasks/{id}` 功能。

## Classic Outlook：本機預覽與明確送出

使用者已確認公司一律使用 Classic Outlook。本版以 Rust `windows` 套件呼叫 Outlook COM，不需要 Python、pywin32、Outlook 外掛或 Graph 授權。

1. 開啟 Classic Outlook，選取**一封**一般郵件，或開啟該郵件視窗。
2. 在 LM_AI 的 Outlook 助理按「讀取選取郵件」。App 只連已啟動的 Outlook，讀取主旨、寄件者名稱、收件者、副本、收件時間與未讀狀態。
3. 此時只在畫面預覽，尚未傳给 AI。內部 EntryID 不傳到網頁或 AI。
4. 使用者按分析並確認後，才將預覽欄位放入一般 Chat Completions 的 user message。
5. 需要正文時，使用者勾選正文並確認；App 再比對同一個 EntryID，選取變動則拒絕，要求重讀預覽。讀取純文字 Body，最多 40,000 UTF-8 bytes，不讀附件。
6. 使用者再次確認分析，才送出含正文的預覽。AI 說需要更多資訊不會自動讀取更多郵件。

不寄信、不刪信、不移動、不修改未讀狀態、不背景掃描信箱；不讀取整個討論串。COM 讀取在專用 STA 背景執行緒，避免阻塞聊天介面。
Outlook 的安全提示由使用者處理，App 不略過保護；公司原則、不同權限層級或無法取得活動 Outlook COM 物件時，介面會提示重新開啟／確認權限。

送給 AI 的郵件資料例如：

```json
{
  "subject": "下週專案進度確認",
  "sender": "示範同事",
  "to": "示範使用者",
  "cc": "",
  "received_at": "2026/09/17 09:00",
  "unread": true,
  "body": null
}
```

received_at 為 Outlook 轉出的本機顯示文字，不當作後端事件游標。body=null 明確表示尚未讀取正文。
這份 JSON 包在一般 messages[].content 的分析提示中，並非新 HTTP endpoint。聊天仍為 model alias、Bearer、stream=false；不要要求新的上游 API Key。

提示要求 AI 在 `choices[0].message.content` 回傳下列 JSON **字串**，不改變 Chat Completions 外層：

```json
{
  "category": "needs_more_info",
  "reason": "僅主旨不足以判斷期限與影響。",
  "summary": "郵件涉及下週的專案進度確認。",
  "needs_body": true,
  "requested_context": "需要正文中的期限與待辦內容。"
}
```

category 限 important／needs_more_info／normal；requested_context 可省略。App 將合法結果顯示為可讀的 Markdown；模型沒有遵守 JSON 時顯示原回覆，不把猜測當作工具命令。後端可透過模型提示提高格式遵從率，現階段不要求上游支援 JSON Schema response_format。
分析會開啟新對話，避免帶入不相關歷史。使用者確認傳送的郵件欄位／正文與結果會納入本機加密對話，可在聊天頁刪除；單純預覽不落盤。

## 驗收順序

先驗證登入後取得一則通知，再測 WebSocket 喚醒、斷線期間新增事件、重連補查、重複 id、分頁、過期、跨裝置已讀與越權 id。
Outlook 實機驗證單選、多選、郵件視窗、未讀狀態不變、正文需確認、讀正文前切換郵件、Outlook 未啟動與公司安全提示。
`--demo` 提供通知及虛構 Outlook 預覽，可先看流程；不能代替公司 Outlook 實機與真實後端驗收。
