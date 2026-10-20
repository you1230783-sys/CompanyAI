# 0.8.25 專案文字 tool_calls

使用者已授權編譯與 Git 交付；本契約納入 0.8.25，僅交付 EXE，不製作 NSIS／離線 ZIP。建置與驗證結果另見 [驗證紀錄](VALIDATION_0_8_25.md)。

## 目的與範圍

將工具參數集中於 `src/projects/tools.json`，依 OpenAI Chat Completions 的 `tools[].function` 形狀描述；技能只說明共同流程及必要文件限制。runner 將技能與壓成單行的工具定義放進 system.content。

官方格式參考：[OpenAI function calling](https://developers.openai.com/api/docs/guides/function-calling)。本方案只借用其資料形狀，**不是原生 API 工具呼叫，也沒有啟用 strict／Schema 約束解碼**；是否提高公司模型成功率仍待實測。正式 Schema 增加英文欄位與型別資訊，不能把「統一規格」當成「token 一定減少」。

HTTP 仍沿用既有背景請求與 `skills:false`。不新增 `tools`、`tool_choice`、`response_format`，不傳真正的 role=tool。網頁不用理解工具用途，只需保留桌面 messages 與模型回答正文；如果既有網站會改寫或移除正文內的工具 JSON，仍需確認轉送行為。

## 單輪資料

模型在原本的回答文字內輸出：

```json
{"content":"先讀取文件。","tool_calls":[{"id":"call_001","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"報告.txt\",\"offset\":0}"}}]}
```

桌面轉成既有操作，交由原本的 broker／受限子程序執行：

```json
{"action":"tool","operation_id":"call_001","request":{"tool":"read_file","path":"報告.txt","offset":0}}
```

工具結果包成下列文字，放在既有 user.content；不是 HTTP 的 tool 角色：

```json
{"role":"tool","tool_call_id":"call_001","content":"{\"ok\":true,\"result\":{\"text\":\"文件原文\"}}"}
```

實際結果仍保留全部原始欄位，包括 offset、revision、document 等。有效呼叫原文與工具結果成對保留，沿用既有上下文縮減規則。

## 簡化與相容

- 提示詞只教一種格式；普通操作、交付 `finish` 與詢問 `ask_user` 都使用單一工具呼叫。這兩個名稱是 CompanyAI 自訂函式，不是 OpenAI 內建工具。
- function.arguments 提示為標準 JSON 字串。parser 額外接受完整 JSON 物件，不因少一層字串跳脫就重試；沒有把物件形式宣稱為原生 Chat Completions 格式。
- progress_note 是各工具可選參數；task_summary 是 finish／ask_user 的可選參數。轉換層抽出後交給既有筆記／任務摘要邏輯，不送給檔案工具。
- id 原樣映射 operation_id；同 ID、同工具參數仍由原本 broker 去重，同 ID 不同參數停止。revision、copy_id 與其他來源證據不改寫。
- content 顯示為活動說明；舊有 JSON 前後簡短說明仍相容。空白／裸完成標記、版本、成果檔案及權限仍走既有檢查。
- 舊 action／request 回覆仍接受，但新提示詞不再教第二種格式，避免同時出現兩套操作規格。
- 每輪恰好一項 tool_calls。多項呼叫、混合新舊外層或多個 JSON 全部不執行，沿用最多連續兩次、全任務六次格式修復。未知工具、無效 ID 仍停止。
- parser 只做明確轉換及型別檢查，不另加完整通用 JSON Schema 引擎；檔案版本、scope、文字長度與存取界線等仍由現有工具實作驗證。

完整 system 與虛構首輪請求見 [PROJECT_REQUEST_0_8_25_EXAMPLE.json](PROJECT_REQUEST_0_8_25_EXAMPLE.json)。它是靜態參考，不是公司實際傳輸紀錄。

## 驗證範圍

八項工具協定單元測試涵蓋兩種 arguments、引號／換行／Unicode、摘要雙版本、交付及詢問、空白完成、多操作與混合格式拒絕、新舊映射一致、工具目錄與 HTTP 文字契約。

真實 loopback HTTP 與 AppContainer 整合覆蓋舊格式、新字串／物件參數、讀取與副本修訂交付、詢問，以及多工具回覆拒絕後修復；保留長文件續接、去重、身分、取消、.lmai 與 PDF 快取回歸。

驗證數量、結果及 EXE 雜湊以 [0.8.25 驗證紀錄](VALIDATION_0_8_25.md) 為準。公司 GLM／Gemma 的成功率、token 用量及網站實際轉送仍待公司測試。
