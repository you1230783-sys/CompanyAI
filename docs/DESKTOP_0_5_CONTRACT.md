# LM_AI 0.5：附件、串流、背景任務與估時契約

**LM_AI 0.5 實作契約，2026-09-17。此契約已先行發布供網站同步開發；桌面驗證範圍與公司驗收項目見 [VALIDATION_0_5.md](VALIDATION_0_5.md)。**

本文件新增的路由不取代 0.4 的登入、版本、模型與通知契約；既有內容見 [WEB_INTEGRATION.md](WEB_INTEGRATION.md)。
0.5 主機固定為 `http://lp2-en-server`。下列路徑包含 `/lm_server`，不可省略或再重複加 `/api/v1`。
桌面送出的模型仍是 `fast`、`quality`、`ultra` 等不透明代號；網站端映射真實模型。**quality 也必須經過伺服器 queue，不從名稱推斷免排隊。**

## 1. 本次網站端工作清單

1. 提供獨立 capabilities 路由，回傳登入帳號的不透明識別碼、支援模式、附件規則與是否支援估時。
2. 提供可重複呼叫的 conversation 建立／取得路由。
3. 用 desktop Bearer 權限包裝既有附件處理服務：預約附件 → 上傳原始 bytes → 查轉檔狀態 → 取得短期附件 token。
4. 在現有 Chat Completions 閘道加入 stream／background；保留沒有新欄位的 0.4 sync 呼叫。
5. **stream 與 background 都建立持久 task**，提供查詢、依 client_request_id 找回與取消路由。
6. 以既有事件系統發布任務事件，REST 任務狀態／結果為真相來源。
7. 包裝既有估時邏輯，分別回傳排隊、處理與總時間。

桌面不做文檔轉 MD、OCR 或圖片轉換；全部由網站既有處理服務完成。這版不新增 tools／skills／UNC／RAG 契約。

## 2. 路由總表

所有新路由都要求 `Authorization: Bearer <desktop access_token>`；只有原本的版本／登入等匿名端點維持原規則。

| 方法 | 完整路徑 | 用途 |
| --- | --- | --- |
| GET | `/lm_server/api/desktop/capabilities` | 附件規則、模式、穩定帳號代號 |
| POST | `/lm_server/api/desktop/conversations` | 建立或取得桌面对話 |
| POST | `/lm_server/api/desktop/conversations/{conversation_id}/attachments` | **JSON 預約附件工作**，不在此傳檔案 |
| PUT | `/lm_server/api/desktop/attachments/{job_id}/content` | `application/octet-stream` 原始檔案 |
| GET | `/lm_server/api/desktop/attachments/{job_id}` | 附件狀態與就緒 token |
| POST | `/lm_server/api/desktop/attachments/{job_id}/cancel` | 移除未送出附件時嘗試取消／釋放 |
| POST | `/lm_server/api/desktop/chat/estimate` | 估時，不能執行 AI 或占用 queue slot |
| POST | `/lm_server/v1/chat/completions` | 既有聊天路由，新增執行模式 |
| GET | `/lm_server/api/desktop/tasks/{task_id}` | 持久任務狀態與最終結果 |
| GET | `/lm_server/api/desktop/tasks/by-request/{client_request_id}` | 回應遺失時查回同一個任務 |
| POST | `/lm_server/api/desktop/tasks/{task_id}/cancel` | 嘗試取消排隊／執行中的任務 |
| GET / WS | `/lm_server/api/desktop/events`、`/lm_server/api/desktop/events/ws` | 沿用 0.4 事件補查／即時通知 |

**附件採 JSON 預約 + PUT bytes，不是 multipart。** 這是為了讓桌面在上傳前先保存 job_id；大檔案或連線中斷後能查回既有工作，避免不確定附件是否已送達。網站 wrapper 可在內部沿用現有 multipart／文件處理服務，不需改寫轉檔引擎。

共同 Header：`X-Client-Version: 0.5.0`。一般請求 `Accept: application/json`，串流為 `Accept: text/event-stream`。
不得回傳 HTML 登入頁或 HTTP redirect；桌面不跟隨轉址、不使用網站 cookie，也不接受後端指定任意上傳 URL。

## 3. 能力與附件規則

`GET /lm_server/api/desktop/capabilities`，200：

```json
{
  "contract_version": 1,
  "principal_id": "desktop_user_b67fe812",
  "execution_modes": ["sync", "stream", "background"],
  "timing_estimates": true,
  "attachments": {
    "enabled": true,
    "max_count": 20,
    "max_file_bytes": 52428800,
    "max_total_bytes": 209715200,
    "allowed_extensions": [".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx", ".txt", ".md", ".csv", ".png", ".jpg", ".jpeg", ".webp"],
    "allowed_mime_types": ["application/pdf", "image/png", "image/jpeg", "image/webp"]
  }
}
```

以上大小與副檔名**只是示例，不是要求網站全部支援**。請改成現有一般聊天實際支援的文件與圖片格式，不要宣告處理服務做不到的格式。

- `principal_id`：登入帳號穩定的不透明字串。同帳號 Token 換發後保持相同，不同帳號不得相同；不可使用 email、真實資料庫流水號。桌面用它隔離加密任務紀錄，授權仍由 server 驗證。
- `contract_version` 必須為整數 `1`。桌面應用版本是 `0.5.0`，兩者用途不同。
- 所有 ID（principal／conversation／job／task／client_request_id）使用 `[A-Za-z0-9_-]{1,128}`，不得包含斜線、網址或路徑。
- `execution_modes` 可包含 `sync`、`stream`、`background`。網站未實作完成的模式先不要宣告。桌面首次優先選 stream，其次 background，最後 sync。
- 這份能力清單描述**本帳號可用模型共同支援的能力**。本版不依模型名稱猜功能，網站不應對同一模式的模型選項回傳互相矛盾的能力。
- 文件與圖片**合計最多 20 個／每次訊息**；server 可回傳更低的 `max_count`，桌面取 `min(max_count, 20)`。不是整個對話終身只能 20 個。
- 大小單位是原始檔案 bytes。桌面依 server 回應驗證單檔與合計大小；WinHTTP 單一上傳的技術上限是 4,294,967,295 bytes，UI 會取較小值。沒有寫死 10 MB／50 MB 的公司規則。
- 副檔名以 `.` 開頭，大小寫不敏感。桌面選檔器與前置檢查以 `allowed_extensions` 為準；`allowed_mime_types` 是輔助資訊，瀏覽器提供的 MIME 不能當可信證明。
- **server 必須另驗證實際內容、大小、檔案類型與帳號權限**，不得只相信副檔名或 client MIME。
- 關閉附件時仍回傳整份物件，例如 enabled=false、max_count=0、max_file_bytes=0、max_total_bytes=0、allowed_extensions=[]。
- capabilities 暫時 404／無法連線時，未取得能力的新客戶端仍可使用 0.4 純文字 sync；不會自行假設附件 API 存在。登入後、手動重新整理及定期服務檢查會重查。

## 4. 建立／取得對話

`POST /lm_server/api/desktop/conversations`：

```json
{"client_conversation_id":"ea51749395204821a56ca9f05ca024a1"}
```

200 或 201：

```json
{"conversation_id":"conversation_8a392dd2"}
```

資料庫對 `(owner, client_conversation_id)` 設唯一約束。重試或重開 App 必須回傳原 ID，不能每次建立另一個 conversation。
網站既有一般聊天使用內部數字 ID 時，wrapper 應轉成不透明的 desktop conversation_id，不能直接要求 client 存取內部 ID。

## 5. 附件預約、上傳與轉檔

### 5.1 預約

`POST /lm_server/api/desktop/conversations/{conversation_id}/attachments`，JSON：

```json
{
  "client_attachment_id": "71efc8d8caa84720a63f2636ec44c992",
  "filename": "研究報告.pdf",
  "size_bytes": 1357924,
  "mime_type": "application/pdf"
}
```

回 200／201，採下節的 AttachmentStatus，初始 `state: "awaiting_upload"`。
桌面先保存 job_id，再傳檔案。對 `(owner, client_attachment_id)` 建立唯一約束；同 ID／相同 metadata 重試回原工作與**目前狀態**，已轉檔完成時直接回 ready。相同 ID 但不同 metadata 回 409，不覆寫原工作。

### 5.2 傳原始內容

`PUT /lm_server/api/desktop/attachments/{job_id}/content`

```http
Authorization: Bearer <desktop access_token>
X-Client-Version: 0.5.0
Content-Type: application/octet-stream
Content-Length: 1357924

<原始檔案 bytes；不是 Base64，不是 JSON，不是 multipart>
```

server 必須串流接收、限制長度，完整保存且驗證後才排入轉檔 queue。不要把整個檔案放在 API worker 記憶體中。
不完整的 PUT 不可排入轉檔，也不可把半份檔案當 ready；應保留可重試的 awaiting_upload 狀態。
再次 PUT 相同內容不可重複轉檔；若已接收完整，回傳原狀態。不同內容不得覆寫已被引用的文件。
回 200／202 AttachmentStatus，不要在 PUT 內一直等到整個轉檔完成。

### 5.3 AttachmentStatus

```json
{
  "job_id": "attachment_job_b26818",
  "state": "processing",
  "progress": 45.0,
  "queue_position": null,
  "attachment_token": null,
  "expires_at": null,
  "error_message": "",
  "timing": {
    "estimated_wait_seconds": 0,
    "estimated_processing_seconds": 12,
    "estimated_total_seconds": 12,
    "confidence": "medium"
  }
}
```

state：`awaiting_upload → uploaded / queued → processing → ready`，或 `failed / cancelled`。可略過極短的中間狀態。

- `progress` 為 0–100，未知回 null，不能捏造百分比。`queue_position` 是 **1 起算**，不排隊／未知回 null。
- ready 必須有非空 `attachment_token` 與 `expires_at`。**expires_at 是 Unix UTC 整數秒**，不是 RFC3339。到期時桌面要求重新選檔。
- token 為不透明短期字串，最多 4096 bytes。不能是 UNC／本機路徑、對外下載 URL、資料庫 ID 或上游 API Key。
- token 必須綁定 owner、conversation 與已完成的文件，server 依映射取回轉好的 MD／圖片。
- 失敗回 `error_message`，最多 2000 bytes；可以另外提供 `error_code`，桌面目前顯示 message。不得包含 Token、內部路徑、stack trace 或真實模型名稱。
- 桌面最多同時上傳兩個附件，其餘顯示等待上傳。轉檔與排隊狀態每約 5 秒查詢；連線失敗退避至最多 30 秒。
- **此版待所有草稿附件 ready 後才允許送出聊天**；上傳／轉檔期间可切換對話與進入托盤，不會自動把文件送給 AI。
- 圖片貼上與選檔走完全相同的流程；貼上通常產生 PNG，網站必須在能力中宣告實際可處理的圖片格式。
- 桌面不永久保存原始附件副本。完整接收前的暫存、待上傳／失敗待重試檔案使用 Windows 使用者 DPAPI 加密；確認 server 接收後清除本機暫存。

移除草稿附件時，桌面呼叫 `POST /attachments/{job_id}/cancel`，body `{}`。回 200 AttachmentStatus。只有尚未被聊天引用的附件可取消／釋放；已被引用的文件依 server 留存政策處理。另須清理長期未上傳、未引用與過期附件，不能永久累積孤立工作。

## 6. 估時

`POST /lm_server/api/desktop/chat/estimate` 接受與下節相同的聊天 JSON（包括 conversation、model、messages、attachment_tokens）。
**不得创建聊天 task、保存 user message、呼叫 AI、消耗附件 token 或占用 queue slot。**
200 直接回 Timing，不再包一層 `timing`：

```json
{
  "estimated_wait_seconds": 12,
  "estimated_processing_seconds": 38,
  "estimated_total_seconds": 50,
  "sample_count": 86,
  "confidence": "medium",
  "generated_at": "2026-09-17T10:00:00Z"
}
```

時間為非負秒數，允許小數或 null。沒資料就回 null，不能以 0 假裝立即完成。`generated_at` 是 RFC3339。
total = wait + processing（已知時）。估算是當下參考值，不是 SLA；實際排隊仍由提交時的 server admission 決定。
本版由使用者按「估算時間」發送，輸入／模型變更後丟棄舊估算。
任務／附件的 Timing 可代表「目前預估剩餘時間」，請一致使用這個定義；不要混用已耗時。
桌面以每個工作的 queue_position／timing 顯示排隊，不依賴另一個全域 queue/status API。

## 7. 聊天請求

仍為 `POST /lm_server/v1/chat/completions`：

```json
{
  "model": "quality",
  "messages": [{"role":"user","content":"請整理附件的研究結論。"}],
  "stream": true,
  "execution_mode": "stream",
  "conversation_id": "conversation_8a392dd2",
  "client_request_id": "c86ad2fdf2cb4a24bc005343a621c281e",
  "attachment_tokens": ["opaque_short_lived_attachment_token"]
}
```

| 模式 | stream | 回應 |
| --- | --- | --- |
| 舊版 sync | false | 200 標準 Chat Completions JSON |
| stream | true | 200 `text/event-stream`，同時建立持久 task |
| background | false | 202 TaskStatus，立即釋放 HTTP 連線 |

- 沒有 execution_mode 的舊 0.4 請求視為 sync，不能破壞既有文字聊天。
- 0.5 選 sync 時使用舊請求格式、不帶附件；附件與長任務使用 stream 或 background。
- 不接受矛盾組合（如 background + stream=true），回 400。不要靜默改換模式或自己猜 quality 應採哪個模式。
- messages 仍是 OpenAI role/content 字串陣列，包含本機完整文字對話（最多 20 輪）；attachment_tokens 只屬於**本次最後一則 user message**。只有附件沒有文字時，桌面補入「請分析附件內容。」。
- 網站依 conversation 保存過去附件與訊息的關聯；後續問題仍應沿用同一對話的文件上下文。不要因 client 未重傳過去 token，就遺失先前文件；也不要把完整 messages 再逐則重複新增到網站紀錄。
- 可以沿用網站現有 message/history，但需以 client_request_id 去重本次新增的 user／assistant 訊息。真實 MD、圖片內容與上游格式都只在 server 端組合。
- server 送出 AI 前再次驗證 token 歸屬、期限、conversation、ready 狀態與數量；禁止 client 任意提交 document_id 讀其他人的文件。
- 排隊優先權與容量、fast／quality pool、aging 均由既有 server queue 決定。不要由桌面根據估時自行繞過 queue。

## 8. TaskStatus 與持久性

background 新增回 202，重複請求可回 200／202 的原 TaskStatus：

```json
{
  "task_id": "task_f468db",
  "client_request_id": "c86ad2fdf2cb4a24bc005343a621c281e",
  "state": "queued",
  "queue_position": 2,
  "progress": null,
  "timing": {
    "estimated_wait_seconds": 18,
    "estimated_processing_seconds": 42,
    "estimated_total_seconds": 60
  },
  "result": null,
  "error_message": ""
}
```

state：`queued → running → completed`，或 `failed / cancelling / cancelled`。
GET `/tasks/{task_id}` 及 `/tasks/by-request/{client_request_id}` 都回 **同一形狀的 TaskStatus**。
completed 必須包含可解析的 result：

```json
{
  "task_id": "task_f468db",
  "client_request_id": "c86ad2fdf2cb4a24bc005343a621c281e",
  "state": "completed",
  "queue_position": null,
  "progress": 100,
  "timing": {},
  "result": {
    "choices": [{"index":0,"message":{"role":"assistant","content":"## 結論\n\n已完成分析。"},"finish_reason":"stop"}]
  },
  "error_message": ""
}
```

可附 usage 等標準欄位，桌面目前取 `result.choices[0].message.content`。目前文字回答與整份 JSON 回應需控制於 1 MB 內。

**可靠執行的必要要求：**

- API 驗證通過後，先將 task、client_request_id、請求摘要與 queue 狀態提交資料庫，再回 202／開 SSE。
- `(owner, client_request_id)` 唯一，必須在資料庫／交易層排除同時提交的競態。相同內容重試只回原工作；相同 ID 不同內容回 409，不重跑 AI。
- 採可恢复的持久 worker；不可只用 request coroutine 或單一 process 的 asyncio.create_task()。worker 重啟必須能恢復或把中斷工作明確標記 failed；不能永遠停在 running。
- 先完整保存 assistant message、usage、audit 與 result，再提交 completed，再發出完成事件。
- 任務已建立後的失敗也要保存為 failed，附安全的 error_message；不能只回一次 HTTP 500 然後丟失任務。
- 關閉 App、關閉視窗、最小化、登出與 SSE 斷線都**不代表取消**伺服器任務。
- 所有 task 查詢／cancel／by-request 都驗證 owner。不存在或不屬於本人可统一回 404；不可洩漏其他帳號的存在性。
- `/tasks/by-request/...` 必須在 `/tasks/{task_id}` 動態路由之前或以正確優先權註冊，避免把 `by-request` 當 task_id。
- 至少保留尚未完成任務；建議完成結果保留 30 天。idempotency 記錄保留至結果到期後仍能阻擋舊 ID 再執行，過期請求回 410／明確失效，不得把舊 ID 視為全新工作。
- 若 POST 回應遺失，桌面先用 by-request 查詢；使用者按「重試送出」也沿用原 ID 與原請求，不新增另一份工作。

取消：`POST /tasks/{task_id}/cancel`，body `{}`，回 200／202 TaskStatus。
取消是 best-effort；排隊中可直接 cancelled，執行中先 cancelling，待 worker 釋放 slot 後 cancelled。
若完成與取消同時發生，以資料庫終態為準：已 completed 就回 completed 與 result，不能覆蓋成 cancelled。

## 9. SSE

回 200、`Content-Type: text/event-stream; charset=utf-8`、`Cache-Control: no-cache, no-transform`。
反向代理需停用 response buffering（例如 Nginx `X-Accel-Buffering: no`）。
採 UTF-8，行尾 LF 或 CRLF，每個事件以空行結束。

**先建立持久 task，立即送 task 事件，再於 queue 等待時送 heartbeat／status**；不要等待幾分鐘取得 slot 後才開始回 Header。

```text
event: task
data: {"task_id":"task_f468db","client_request_id":"c86ad2fdf2cb4a24bc005343a621c281e","state":"queued","queue_position":2,"timing":{"estimated_wait_seconds":18}}

: heartbeat

event: status
data: {"task_id":"task_f468db","client_request_id":"c86ad2fdf2cb4a24bc005343a621c281e","state":"running","queue_position":null}

data: {"choices":[{"index":0,"delta":{"content":"你好"}}]}

data: {"choices":[{"index":0,"delta":{"content":"，這是分析結果。"}}]}

event: status
data: {"task_id":"task_f468db","client_request_id":"c86ad2fdf2cb4a24bc005343a621c281e","state":"completed"}

data: [DONE]

```

- task／status 使用 TaskStatus 欄位。SSE 的 completed 可以省略 result，因為桌面會 GET task 取權威結果；REST completed **不能省略 result**。
- delta 沿用 OpenAI `choices[0].delta.content`，不包成自訂文字事件。可送 role／finish_reason／usage，但桌面目前只顯示 content。
- 等待／執行時每 15–20 秒至少 heartbeat 一次；桌面接收 idle timeout 為 60 秒。
- `[DONE]` 前必須先保存完整結果。桌面不以 `[DONE]` 或部分 delta 當作已保存的完整回覆。
- 可以送 `event: error` + JSON；同時將持久 task 設為 failed，讓斷線後查詢仍能得到錯誤。
- 缺少 `[DONE]`、HTTP 中斷、格式錯誤或串流持續超過約 10 分鐘，桌面保留目前文字並轉 REST 查詢，**不自行重跑 AI**。
- stream disconnect 不取消 worker；server 必須把生成與 SSE 訂閱的生命週期分開。使用者想取消時需呼叫 cancel API。
- 此版不要求 SSE Last-Event-ID／delta replay。重新開啟 App 後可以只顯示任務進度，完成後取得全文。

## 10. 事件與通知

沿用 0.4 envelope、cursor 與已讀 API，不新增另一套 WebSocket 認證。事件例子：

```json
{
  "id":"event_879d",
  "type":"chat.completed",
  "title":"分析完成",
  "summary":"你的背景分析已完成。",
  "created_at":"2026-09-17T10:02:00Z",
  "expires_at":null,
  "resource_id":"task_f468db",
  "read_at":null
}
```

建議事件：`chat.queued`、`chat.started`、`chat.progress`、`chat.completed`、`chat.failed`、`chat.cancelled`；附件採 `attachment.queued / progress / ready / failed`。
resource_id 指向本人可查詢的 opaque task_id／job_id，不放任意工具指令、URL、UNC 或郵件 ID。
事件裡不放完整回覆、附件 token 或原始文檔，只作通知；查詢任務才取得結果。
進度事件應節流，避免每個 delta 都產生持久事件。completed／failed／cancelled 必須可 REST 重播。

WebSocket 收到非 ping 訊號會喚醒桌面補查事件與任務；沒有 WS 也以 REST 輪詢工作。
桌面在結果保存本機後提示一次，chat.*／attachment.* 事件不另外重複彈出同一份完成提示。
真正退出 App 時不會有桌面即時通知；重新啟動後再同步。最小化至托盤仍會查詢並可提示。

## 11. 錯誤與相容性

共同錯誤形狀：

```json
{"error":{"code":"attachment_not_ready","message":"附件尚未處理完成。"}}
```

401 需重新登入；403 權限不足；404 不存在／非本人；409 ID 衝突；413 超限；415 不支援格式；422 欄位無效；426 客戶端需更新；429 暫時過載。
錯誤不能回傳內部 exception／URL／真實模型／憑證。server 不得因 client 提交 queue_priority 或 user_id 就相信它；桌面也不會提交這些欄位。

先部署新路由與持久 worker，完成下列驗收，再在 capabilities 宣告 stream／background／attachments.enabled。
版本檢查維持 latest_version／minimum_version，功能尚未完整部署時不要提高 minimum_version 強迫使用者下載尚不能串接的版本。

## 12. 網站端驗收清單

- 舊 0.4 stream=false 請求仍能回覆，登入／模型／通知不退化。
- 同 owner／client_conversation_id 重試回同一 conversation；不同 owner 隔離。
- 20 個文件＋圖片成功，第 21 個被拒絕；單檔、合計大小及真實格式都由 server 驗證。
- 預約回應遺失後以同 client_attachment_id 重試，仍只一個 job；PUT 中斷不開始轉檔；重試同內容不重複處理。
- queued／processing／ready／failed 状態可觀察，ready 的 token 與期限有效；不同使用者／對話的 token 被拒絕。
- 估時未知回 null，queue wait 與 processing 分開；估算本身不建立任務。
- stream 的中文可跨 TCP chunk，前端逐段顯示；排隊期間 heartbeat 可通過代理；AI 完成後 REST 結果與全文一致。
- background 立即回 202；關閉桌面／重啟 API worker 後仍可查到任务結果。
- 重複 POST 同 ID、兩次並行 POST 同 ID，只執行一次 AI；同 ID 改 body 回 409。
- 在 server 接受後刻意丟棄 POST 回應，by-request 仍能找回；再次提交同 ID 不新增工作。
- 截斷 SSE 或省略 DONE，task 仍保存結果，桌面可 REST 取回；不把部分回答標成 completed。
- 排隊取消、執行取消、取消與完成競態符合終態規則，queue slot 不洩漏。
- events 補查與重新連線可去重；事件游標落後不影響 task REST 真實狀態。
- 重新登入同帳號能恢復工作；不同帳號不能查前一帳號的 task／attachment／conversation。

桌面本機模擬後端只供測試流程，不具備資料庫持久 worker，不能將其當作公司後端部署範例。
