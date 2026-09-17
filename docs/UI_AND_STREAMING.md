# 介面與串流修正

## 顯示與操作

- 上方標題列縮為 48 px，移除不必要的標籤與附件規則說明；上傳時仍依目前模型 capabilities 檢查格式、大小及合計數量，規則取得失敗仍會提示。
- 回覆依序出現 Answer、Key points、Sources 時，從 Sources 起放入預設收合的「來源、信心與限制」。支援 Markdown 編號清單、標題及獨立粗體段落；普通回答與程式碼不因此裁切。展開可閱讀原內容，複製仍使用完整原文。
- 進入任務後切換到其他頁，清除已完成且已保存回答的本機任務卡。失敗、取消、進行中或尚未保存回答的任務保留；不刪除對話或伺服器任務。
- 進入通知後切換到其他頁，將 AI 通知標記已讀並同步，同時請求全站通知 ReadAll。若網站同步或個別操作正在進行，等待結束再送出；網站回覆成功才更新網站通知的本機狀態。
- 設定新增深色模式，使用灰藍背景而非純黑，包含內容區、側欄、設定、輸入欄與標題列，偏好會保存。舊偏好預設維持淺色。
- 設定的「本機資料」明示：伺服器仍保存已提交的資料，本機清除不等同刪除伺服器資料。

## 串流契約

以下為網站格式，所有事件以空行結束：

```text
event: start
data: {"event":"start"}

event: tool_status
data: {"event":"tool_status","status":"started","tool_name":"search_session_documents","arguments":{}}

event: tool_status
data: {"event":"tool_status","status":"completed","tool_name":"search_session_documents","result":{}}

event: delta
data: {"event":"delta","text":"已收到的文字"}

event: done

```

`start` 也可省略 data。工具狀態在首段文字之前即可顯示；名稱與狀態使用安全純文字，不硬編碼中文工具名稱。畫面顯示最近一次狀態，arguments／result 不傳入前端、不保存為工具紀錄。OpenAI 的 `choices[0].delta.content` 與 `[DONE]` 繼續支援。

串流中斷時，已收到的文字保留並加上不完整提示；本機加密歷史與任務紀錄保存這段內容。REST 仍是完整結果的依據，取回全文後取代同一筆部分回答。若任務最終失敗或停止追蹤，部分回答仍可在對話中閱讀。這是連線中斷處理，不保證程式被強制終止瞬間尚在記憶體的每個 delta 都已落盤。

## 驗證範圍

Rust 測試包含具名事件與 OpenAI 格式、UTF-8 分段、工具名稱與資料隔離、部分回答序列化／還原／取代與去重、任務清除條件及舊設定相容。Loopback HTTP 測試混合兩種 delta、工具開始／完成與具名 done，並模擬斷線後 REST 取回。

WebView2 自我檢查涵蓋三種回覆格式收合、程式碼不誤判、頁面離開命令只送一次、附件說明隱藏、深色背景、首段文字前工具狀態與完成更新，以及不完整回答警示。

公司仍需以真實串流、通知服務及 Outlook 2024 的 Exchange／PST 帳號實測。本機模擬與介面檢查不代表公司服務已驗收。
