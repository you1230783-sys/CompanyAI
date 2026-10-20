# LM_AI 0.7.0 桌面與網站契約

本版保留既有 `/lm_server` 部署前綴。這份文件記錄桌面已採用的請求與更新欄位；公司服務是否已部署，仍需網站端確認。

## 聊天與用途

`POST /lm_server/v1/chat/completions`，`Authorization: Bearer <desktop-token>`、`Content-Type: application/json`；`X-Client-Version` 使用實際程式版本 `0.7.0`，不固定偽裝成範例的 `0.4.0`。

| 介面用途 | stream | execution_mode | outlook_triage | auto_generate_title |
| --- | --- | --- | --- | --- |
| 一般聊天（SSE） | true | stream | false | false |
| 背景聊天 | false | background | false | false |
| Outlook 初篩 | false | background | true | false |
| 對話標題 | false | background | false | true |
| Outlook 附件最終整理 | 依模式 | background 優先 | false | false |

兩個用途旗標不能同時 true。單封 Outlook 判讀也帶 `outlook_triage=true`；不自動額外產生標題。一般聊天的首次問題另送快速模型產生最多 15 字標題，這是獨立的背景任務與伺服器對話，不能把標題指令混入原聊天上下文。標題／置頂只保存在本機；使用者手動改名優先於晚到的自動標題。

聊天仍帶 `conversation_id`、固定的 `client_request_id` 與本次 `attachment_tokens`。網站需提供原有持久任務與 REST 查詢。背景工作不能只活在 HTTP 請求 coroutine。重試依 owner + request ID 去重，不重跑模型。

「翻譯／摘要／潤飾」可選擇只在該次請求使用 fast，平常模型偏好不變。桌面對不同模型重新查 capabilities 與附件規則；網站仍需驗證模型權限與附件內容。

## SSE

採 `text/event-stream; charset=utf-8`，UTF-8 單行 JSON，每個事件以空行結束。Gateway 立即轉送內容事件，不等全文完成、不重複送同一片段。

```text
event: task
data: {"task_id":"task_123","client_request_id":"request_123","state":"queued"}

event: status
data: {"task_id":"task_123","client_request_id":"request_123","state":"running"}

event: start
data: {"event":"start"}

event: tool_status
data: {"event":"tool_status","tool_name":"search_session_documents","status":"started"}

event: delta
data: {"event":"delta","text":"新增片段"}

event: tool_status
data: {"event":"tool_status","tool_name":"search_session_documents","status":"completed"}

event: status
data: {"task_id":"task_123","client_request_id":"request_123","state":"completed"}

event: done
data: {"event":"done"}

```

`task`／`status` 直接使用 TaskStatus，不能多包一層。來源的 complete／completed／done／[DONE] 需等完整結果保存後，由 Gateway 統一輸出 done。桌面兼容上述完成名稱與舊 OpenAI delta.content，但網站不應同時輸出兩套文字片段。

每 15–20 秒至少發送一次 `: heartbeat\n\n`，排隊也要維持連線。反向代理與應用層不得緩衝完整回覆。桌面已改用 WinHttpQueryDataAvailable 取得可讀長度後再讀取，避免小事件等候 8 KB 緩衝區。

REST `GET /lm_server/api/desktop/tasks/{id}` 與 `/tasks/by-request/{client_request_id}` 維持既有 TaskStatus；完成時 `result.choices[0].message.content` 必須含完整回答。SSE completed 可不含 result。done 不取代 REST 的保存確認；斷線不取消 worker、不自動重跑。

工具狀態只顯示／保存 tool_name、status，不將 arguments／result 交給前端。背景模式若網站沒有對應工具事件，桌面只能顯示網站提供的任務狀態，不能臆造工具進度。

## Outlook JSON 容錯

初篩接受自然語言或 Markdown 中夾帶的一份 `{schema_version, summary, requests}` JSON。找到多份決策、欄位不符、未知工具、批次外 ID 或超出授權時，保留原始回答並停止匯出，不猜測命令。只有整份決策通過原有授權驗證才執行 `outlook.export_msg`。

初篩本身是持久背景任務；程式關閉後仍可補查原始回答，但本機 COM 匯出權限不會跨重新啟動自動恢復。附件最終整理維持一般自然語言可讀結果。

## 安裝與更新規格已由 0.8 取代

請改用 [UPDATE_0_8_CONTRACT.md](UPDATE_0_8_CONTRACT.md)。不再使用 PowerShell 更新助手、舊版 version.update 欄位或退出時自動套用；下載檔改為完整 NSIS Setup。聊天、SSE、Outlook 與本機對話規格仍適用。
