# LM_AI 0.6：全站鈴鐺、模型附件規則與 Outlook 自動補充

2026-09-17。本文件先行提供網站同步開發；文件提交不表示 EXE 已完成驗收。交付驗證以 `offline/verification.json` 及 [驗收說明](VALIDATION_0_6.md) 為準。
沿用 [0.5 契約](DESKTOP_0_5_CONTRACT.md) 的認證、附件、聊天與持久任務；本次只修改桌面專案，不修改網站原始碼。

## 網站本次要做的事

1. capabilities 接受 `model` query，回傳該模型可接受的附件規則。
2. 新增 desktop Token 版本的全站鈴鐺 API，共用既有看板／新報通知資料、所有權、已讀與刪除。
3. 通知同步必須包含新增、已讀變更與刪除；提供 `deleted_ids` 及失效游標的 410。
4. 網站與桌面任一端修改通知後，以既有 WebSocket `events_available` 喚醒桌面 REST 同步。
5. 允許的模型若支援 `.msg`，將它接入既有文件轉換服務，取得正文、圖片及附件內容；沿用 0.5 上傳路由，不另建 Outlook 上傳 API。
6. 聊天閘道需保留用戶要求的初篩 JSON，不要注入強制覆蓋此格式的提示詞。網站不執行 Outlook 工具，COM 與工具指引均由桌面負責。

所有下列路徑都包含目前部署前綴 `/lm_server`。認證沿用 `Authorization: Bearer <personal_access_token>`，所有請求包含 `X-Client-Version: 0.6.0`。不要拿網站 Cookie 路由直接充當 desktop Token API。

## 1. 依模型查詢附件規則

`GET /lm_server/api/desktop/capabilities?model=fast`

回應形狀沿用 0.5；`attachments` 與 `execution_modes` 必須適用於指定 model alias。未知／無權使用模型回 400／403，不應默默套用其他模型。

```json
{
  "contract_version": 1,
  "principal_id": "opaque_account_id",
  "execution_modes": ["sync", "stream", "background"],
  "timing_estimates": true,
  "attachments": {
    "enabled": true,
    "max_count": 20,
    "max_file_bytes": 10485760,
    "max_total_bytes": 52428800,
    "allowed_extensions": [".pdf", ".docx", ".txt", ".md", ".msg"],
    "allowed_mime_types": []
  }
}
```

支援圖片的模型可以增加 `.png`、`.jpg` 等。App 切換模型立即查詢，舊模型較晚回覆不能覆寫新模型規則。查詢期間停止新增附件及送出；已存在的不相容附件保留並顯示原因，使用者移除或切回模型即可。

規則只用於預檢，實際上傳及聊天仍由後端驗證。PDF／MSG 可能產生圖片：**若模型不能接收圖片，後端必須使用可支援的文字結果，或清楚拒絕，不可把圖片轉發給不支援的模型。** App 不知道伺服器轉檔結果的實際格式。

## 2. 全站鈴鐺 REST API

路由集中於 `src/site_notifications.rs`，正式網址固定，不提供使用者編輯。

| 方法 | 路徑 | 行為 |
|---|---|---|
| GET | `/lm_server/api/desktop/notifications` | 同步全站鈴鐺 |
| POST | `/lm_server/api/desktop/notifications/{notification_id}/read` | 將本人通知標為已讀 |
| POST | `/lm_server/api/desktop/notifications/read-all` | 本人全站通知全部已讀 |
| DELETE | `/lm_server/api/desktop/notifications` | 刪除本人全站通知 |

GET query：`after` 為 opaque cursor，可省略；`limit=100`。可由網站額外支援 `unread_only`，但桌面同步不使用，以免漏掉已讀狀態與刪除。

```json
{
  "notifications": [
    {
      "id": "kanban:123",
      "source": "kanban",
      "type": "mention",
      "title": "你被提及",
      "body": "通知內容",
      "url": "/lm_server/kanban/board/1",
      "resource_id": "123",
      "created_at": "2026-09-17T10:00:00Z",
      "is_read": false,
      "read_at": null
    }
  ],
  "deleted_ids": [],
  "next_cursor": "opaque_cursor",
  "has_more": false,
  "unread_count": 3
}
```

- `id` 是不透明字串，支援 `kanban:123`／`briefing:456`；桌面作單一路徑段編碼，不解析資料庫 ID。後端每次依 Token 驗證所有權，不能相信 client ID。
- `source` 為來源標籤，如 `kanban`／`briefing`；桌面 domain 再加 `source=site` 和 `notification_key=site:<id>`，AI 則為 `ai:<event_id>`。
- `is_read` 為權威值，包含網站端操作後的變更；`read_at` 可為 null。時間採 RFC3339 UTC。
- `deleted_ids` 是自上次游標以來已刪除的 ID。**若只有新增資料，桌面無法得知網站刪除，請務必提供刪除紀錄。**
- 每頁 `notifications`／`deleted_ids` 各不超過 100；`has_more=true` 必須有不同的 `next_cursor`。`unread_count` 是該帳號全站未讀總數，不只是此頁筆數。
- 無 `after` 表示完整快照；多頁應具一致快照／水位，避免完整同步期間發生修改而漏失通知。快照結束游標可接續增量變更。
- 410 表示游標失效。桌面重新做完整同步，所有頁成功才替換快取，失敗保留原內容。後端刪除紀錄若已清除，也應讓過旧游標回 410。
- 操作回 200 或 204 即代表網站資料已提交。刪除／全部已讀涵蓋操作當下已有的本人通知；操作後的新通知不應被延遲操作回應吞掉。

## 3. 同步、通知與開啟連結

- 網站鈴鐺與 AI events 使用不同快取、游標與錯誤訊息；共用現有 `/lm_server/api/desktop/events/ws`，收到喚醒後兩邊各查 REST。
- 啟動、恢復前景、WebSocket 喚醒／重連時同步；正常每 60 秒輪詢。錯誤退避至 600 秒；429 至少等待 120 秒。401／403 停止該來源的自動重試，重新登入後恢復。
- 第一份完整快照／游標失效後重新完整同步不跳歷史氣泡。後續新 ID 才提示，重播不重複。App 在前景時不發系統氣泡。
- **全站通知不因 App 在前景就自動已讀**，避免改變網站鈴鐺語意。明確點擊／全部已讀且 API 成功才更改。
- 點擊全站通知先呼叫 read API，成功後才開 URL。失敗保留未讀狀態；開啟連結失敗不撤銷已成功的網站已讀操作。
- 目前看板／新報無桌面對應頁面，使用系統瀏覽器。URL 只接受同一公司 origin 的 HTTP(S)，不夾帶 Token；非 HTTP、跨站、帳密 URL、控制字元均拒絕。
- `"kanban/..."` 相對路徑從 `/lm_server/` 解析；`"/lm_server/kanban/..."` 是完整 origin-relative 路徑。**請網站不要省略 root-relative URL 的部署前綴**。不使用 `/lm_service`。
- 本機加密快取最多保存 500 則網站通知，未讀 badge 使用後端總數；完整同步最多 100 頁。超限不破壞舊快取，網站應合理限制通知保存期間。

## 4. AI 通知及卡住任務

- 通知中心只顯示 AI 成功回覆與失敗：`chat.completed`／`chat.failed`（相容 `task.*`／`ai.*` 的 completed／failed）。附件排隊、上傳、轉檔只顯示在卡片，不發通知。
- AI event cursor／read API 保留。清除 AI 通知只隱藏本機紀錄，**不冒充網站鈴鐺的刪除**；全站鈴鐺則一定呼叫 DELETE API。兩來源操作失敗各自顯示。
- 工作任務提供「取消任務」（等待 server 確認）、「停止追蹤」（立即解除本機等待）及「移除任務」（保留對話，移除本機卡片）。
- 404、離線或回應遺失皆能停止／移除；App 另行嘗試依原 request_id 找回及取消。**本機停止不保證 server 已停止**。晚到的回應不能復活已停止／已移除任務。
- 停止接收串流在下一段資料／heartbeat 或網路逾時結束。已接受的 server 工作若取消失敗仍可能執行完成，不自動重送新請求。

## 5. Outlook 本機 Skill 與自動補充

技能原文：[src/outlook/skill.md](../src/outlook/skill.md)。不需要網站提供新的工具執行器。

1. 使用者可選 Outlook 多封郵件，或按「所有／僅未讀 × 今天／三天內／本週」。日期查詢預設涵蓋已載入信箱與本機資料檔及子資料夾；也可改查目前資料夾或預設收件匣及其子資料夾。三天含今天，本週從週一開始，使用本機日曆日期。完整範圍與上限見 [Outlook 查詢說明](OUTLOOK_SEARCH.md)。
2. 每批最多 50 封，可勾選子集；截斷有提示。只讀取主旨、寄件者、收件者、副本、時間、未讀狀態，不讀正文。
3. 使用者確認分析。若勾選「自動補充」，此授權僅涵蓋本批勾選郵件，可由 App 依 AI 請求匯出完整 MSG（正文、圖片及附件）並傳到公司網站。
4. 初篩使用既有 sync Chat Completions，`stream=false`，把 Skill 與資料作為文字送出。回應 `choices[0].message.content` 必須是下列 JSON 字串；不是 OpenAI `tool_calls` 欄位。

```json
{
  "schema_version": 1,
  "summary": "先根據基本資訊整理，說明尚待確認的部分。",
  "requests": [
    {
      "tool": "outlook.export_msg",
      "mail_id": "本批桌面提供的不透明代號",
      "reason": "需要確認期限及附件內容"
    }
  ]
}
```

5. App 嚴格檢查整份 JSON，只允許這一個工具、本批勾選 ID、每封一次，數量不超過後端規則及 20。未知工具、額外欄位、未知 ID、重複 ID、未授權要求一律不執行。
6. 原生 COM 使用保存於記憶體的 EntryID＋StoreID 取得原郵件，匯出 Unicode MSG。這些 Outlook 內部 ID 不傳 AI 或 WebView。AI 不能指定磁碟路徑、網址或程式。
7. MSG 先在 App 私有子目錄產生，轉成 DPAPI 區塊暫存後清除明文副本。由既有附件路由上傳，網站負責文字／圖片轉換。不是單獨抽出原附件讓使用者手動上傳。
8. 全部附件 ready 後，App 自動以「品質」模型 `quality` 送出最終整理，依該模型能力優先 background，否則 stream，沿用 0.5 持久任務／取消／恢復機制。提示詞逐一列出上傳檔案換成 `.md` 後的完整名稱，要求使用網站文件工具讀取；不再要求桌面 Outlook 匯出工具。一般聊天的模型偏好不變。
9. 使用者可停止初篩／匯出／等待轉檔的後續自動步驟。已開始的 HTTP 或 COM 呼叫需等返回；停止不能撤回已傳的資料。最終聊天提交後，改在任務面板停止／移除。
10. 初篩與 COM 授權只在本次 App 執行有效，退出後不自動恢復信箱存取。已保存的附件可在對話查看；已提交最終任務仍可恢復查詢。

**網站只需支援 MSG 轉檔與既有聊天／附件 API。** 郵件是未信任內容，不能覆蓋 Skill 或擴大工具權限。整個流程不寄信、不刪信、不改已讀、不移動郵件。

### 初篩與附件分析的分流

- 第一輪仍以目前選取的模型送出，使用者訊息以 `# Outlook 郵件初篩 outlook-triage-v1` 開頭。網站依此辨識初篩，分派到不額外附帶網站提示詞／技能的處理路由，讓桌面收到約定的 JSON。
- 第二輪由網站自動分派到附帶提示詞／技能的正常分析路由。桌面不加入或猜測新的 URL；本機對話只保留基本資訊與初篩摘要，不把第一輪 Skill、路由標記或「只回 JSON」規則重送給第二輪。
- 例如實際上傳 `mail_abc.msg` 與 `mail_def.msg`，第二輪提示詞的 JSON 檔名清單為 `["mail_abc.md","mail_def.md"]`。原始上傳名稱、MSG 位元組及 attachment_tokens 不變。此契約依網站將最後一段 `.msg` 副檔名替換為 `.md` 的命名方式。
- 自動補充開始前，桌面確認可用模型清單包含 `quality`，另查 `capabilities?model=quality`；使用品質模型的格式、大小、數量與執行模式限制，並核對 principal_id。品質模型不可用時提示錯誤，不改用其他模型。送出前再次以該能力資料檢查全部附件。

## 官方 COM 參考

- [NameSpace.GetItemFromID：EntryID 與 StoreID](https://learn.microsoft.com/en-us/office/vba/api/outlook.namespace.getitemfromid)
- [MailItem.SaveAs](https://learn.microsoft.com/en-us/office/vba/api/outlook.mailitem.saveas)
- [olMSGUnicode = 9](https://learn.microsoft.com/en-us/office/vba/api/outlook.olsaveastype)
- [Items.Sort](https://learn.microsoft.com/en-us/office/vba/api/outlook.items.sort)
