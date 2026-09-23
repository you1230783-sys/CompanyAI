# 結構化回覆與舊格式備援

桌面依使用者提供的網站規則與背景回答範例，優先讀取固定欄位，不再只保存 Chat Completions 的正文。背景與串流任務完成後都經過 `TaskStatus::reply()`；任務識別碼、帳號隔離與 REST 完成狀態仍依既有契約驗證。

## 欄位與顯示

| 欄位 | 型別 | 顯示方式 |
|---|---|---|
| answer | 非空字串 | Markdown 正文，固定可見 |
| sections.key_points | 字串陣列，可省略 | 回答重點，固定可見 |
| sections.sources | 字串陣列，可省略 | 收合區的來源摘要 |
| sections.confidence | 字串或 null，可省略 | 收合區的信心度 |
| sections.limitations | 字串陣列，可省略 | 收合區的回答限制 |
| citations | 陣列，可省略 | 收合區的引用文件 |

`sections` 可省略或為空物件。空陣列及空字串不產生空標題。只使用 `sections.confidence`；最外層 confidence 的語意可能不同，不拿它覆寫或補入顯示信心。引用字串經 Markdown 安全渲染，尚未限定 schema 的引用物件以完整唯讀文字保留，不猜測檔案路徑、不自動執行或開啟。所有正文、條列與連結仍經既有 Markdown／DOMPurify 處理，遠端圖片不自動載入。

`usage`、`risk_level`、`masked_entities`、`tool_calls`、`images`、`reference_images` 不轉成此功能的 UI 操作，也不加入正規化的顯示 DTO。既有原始任務 result 仍按原本方式加密保存；本次沒有新增圖片顯示或工具執行能力。payload 內的 request_id／session_id／message_id 不用來取代桌面的任務關聯檢查，範例中的空 request_id 不影響已驗證的 client_request_id。

## 支援的結果包裝

既有 TaskStatus 的 task_id、client_request_id、state 保持原契約。已完成任務支援下列結果形式：

- `result` 直接為上表的 payload 物件。
- 任務最外層或 `result` 的 `response_payload_json`，可為物件或序列化 JSON 字串。
- `result.choices[0].message.response_payload_json`，可為物件或序列化 JSON 字串。
- `result.choices[0].message.content` 是完整 payload 的 JSON 字串。
- 舊的 `result.choices[0].message.content` 純文字／Markdown。

明確的 response_payload_json 欄位優先，其次是直接 payload，最後才嘗試 content 內的完整 JSON。沒有有效 payload 時保留舊 content；沒有任何可讀回答時回報格式錯誤，不產生空白成功訊息。只檢查固定位置，不遞迴搜尋任意 JSON 或執行其中指令。

舊格式備援只辨識編號章節（如 `1. 回答：`、`## 2. Key points`），標籤與使用者提供的網頁清單一致，見 [介面與串流](UI_AND_STREAMING.md)。一般正文碰巧提到 Sources、信心度等字詞不會觸發收合。0.8.11 支援標題與內容同列，以及使用者回報的五段英文 Markdown 壓成單行。Rust 只在 Answer 開頭、五種章節各出現一次時正規化；不從引用、程式碼或說明前言猜測欄位。若 answer 本身是這種完整五段全文，也會拆出正文並補空欄位，既有非空 sections 仍優先。

## 串流、保存與複製

串流期間沿用現有 SSE delta 文字顯示；最終仍以 REST 的完整結果為依據，與背景任務共用解析。此變更不推定服務會在每個 delta 重送完整 JSON，也不在 JSON 還不完整時猜測欄位。收到完成 payload 後，一次將正文、重點與可展開內容寫入對應訊息，取代同 request_id 的部分回答，重播不新增重複回答。

本機 Message 新增可省略的 `response_payload`，只含上表欄位；舊歷史沒有此欄位仍可讀取。其 `content` 會產生包含所有可讀欄位的 Markdown，供複製及後續聊天 role/content 使用；前端顯示直接使用 response_payload，不反向解析這份 Markdown。標題生成與 Outlook 工具初篩仍只取原始 answer，避免顯示用章節干擾其專用格式。

0.8.11 補強完成切換：只有 answer 的 payload 不代表其他欄位已完整。正文一致（忽略重複空白）時，從較低優先序結果或同任務串流已收到的完整五段回覆補齊空欄位，不覆寫 REST 的非空欄位。若正文改變、欄位衝突或原文格式無法辨識，透過可省略的 `received_replies` 保存另列文字，畫面在「來源、信心、限制與原文」區塊供點擊展開；複製文字也包含這些內容。結構化 JSON 的備份只含可讀欄位，不納入內部工具資料。這項保留不改變任務完成判定，也不會把未完成的串流直接當成 REST 完成結果。

本次不自動重新查詢或改寫已保存的舊正文訊息。新收到的結果會保存完整結構，重新開啟對話後仍可展開閱讀。測試範例在 `ui/fixtures/structured-reply.json`，與 Rust 協定測試、WebView2 自檢共用；驗證記錄見 [回覆章節驗收](VALIDATION_REPLY_SECTIONS.md)。
