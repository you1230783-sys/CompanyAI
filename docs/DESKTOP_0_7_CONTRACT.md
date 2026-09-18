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

## 自動更新：版本服務的選用欄位

既有 `GET /lm_server/api/desktop/version` 加上 `update`：

```json
{
  "latest_version": "0.7.1",
  "minimum_version": "0.7.0",
  "message": "有新版本可用",
  "update": {
    "version": "0.7.1",
    "url": "/lm_server/desktop/releases/0.7.1/LM_AI.exe",
    "sha256": "填入正式簽章完成後之 EXE 的 64 位十六進位 SHA256"
  }
}
```

上例下載路由是建議位置，並非宣稱網站已部署。網站可提供另一個同來源的直接 EXE 路徑。下載需直接回 200，不是 HTML 下載頁、不重新導向；此版更新檔下載不帶登入 Token，因此需提供公司網內可直接取得的已簽章檔案。

自動更新需要目前安裝的 EXE 與新 EXE 均有 Windows 驗證通過、相同憑證 thumbprint 的 Authenticode 簽章。另驗證 SHA256、ProductName=LM_AI、ProductVersion 對應新版本，且拒絕降版。只有 HTTP 來源的 SHA256 不能作為來源信任依據。

未提供 update 欄位或有效簽章時，程式會明確說明不能啟用自動更新，不執行未驗證的下載檔。憑證更換需由 IT 重新部署第一版受信任版本。本次沒有取得公司程式碼簽章憑證；未簽章測試版不能宣稱已完成正式自動更新驗收。

下載完成提示使用者立即重新啟動，或真正退出時套用。關閉視窗／最小化只縮到托盤。更新助手等待原程序退出，在同目錄原子替換 EXE、保留 previous 備份；替換或啟動程序失敗時嘗試還原。正在處理郵件 COM／接收附件時不突然重啟。

## 安裝與資料

`LM_AI_Setup.exe` 內嵌 LM_AI 與 WebView2 x64 離線安裝檔。安裝於 `%LOCALAPPDATA%\Programs\LM_AI`，建立目前使用者的桌面／開始功能表捷徑及解除安裝項目。設定／登入／對話留在 `%LOCALAPPDATA%\CompanyAI`，更新不覆寫使用者資料。

解除安裝可保留本機資料（預設），或由使用者明確選擇刪除；不刪除伺服器資料，也不移除共用 WebView2。產品資訊為 LARGAN、LM_AI、公司 AI 助理（Dev: 1230783），著作權沿用 Copyright © 2026 LARGAN. All rights reserved.

安裝包本身不代替數位簽章。發行時先簽主程式，再用已簽主程式製作 Setup，最後簽 Setup，更新清單雜湊必須取自簽章後的主程式。離線編譯包不包含私鑰或憑證密碼。
