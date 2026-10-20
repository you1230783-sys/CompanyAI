# 0.8.16 專案文件工具與 API 契約

本版為 TXT／MD 可行性測試。交付 EXE、EXE 更新清單及原始碼；先不重製 NSIS 或歷史離線 ZIP。

## 使用方式

1. 登入後，在 VNC 下方「專案」按新增，輸入名稱並使用 Windows 選擇器指定本機資料夾。不能指定整個磁碟、UNC 或包含重新解析點的路徑。
2. 在專案內建立對話，要求讀取或修訂某個相對檔名，例如「請讀取 report.txt，將第二段改得簡潔一點，另存 TXT 副本」。專案對話自動請求標題；一般對話不帶文件工具。
3. 成果在專案 `_AI_Output/<本次任務識別碼>/<唯一識別碼>_<檔名>`；對話列出相對路徑。原始文件不取得寫入權限。
4. 若讀取失敗，先用公司核准的記事本開啟，在專案「匯入文字」填相對檔名並貼上全文。這是使用者提供的明文快照，以 DPAPI 保存，不會自動跟隨原檔變更。後續原檔更新要重新匯入或清除快照。
5. 任務可停止；一次最多一個專案任務。一般聊天可繼續使用。移除專案只撤銷授權，對話移至最近對話，文件不刪除。

**加密相容性限制：** 本版不自動控制記事本／Notepad++，也不保證普通檔案 API 能得到公司明文。編碼解碼不能代替解密；密文可能沒有可靠的可辨識標記。顯示文字仍需使用者核對。輸出後使用相同受控讀取路徑重新驗證，若公司加密攔截造成無法讀回，回報未交付並保留可能已建立的檔案，不能把檔案存在當完成。

MD 在公司不加密：一般新文件只建立 TXT；TXT 來源維持 TXT。讀取 TXT 後，當次任務禁止發布 MD，避免把受保護文字轉為未加密文件。已有 MD 的獨立修訂支援 MD。README／開發說明用 MD，不放公司文件內容。

## 後端請求

路徑仍為 `POST /lm_server/v1/chat/completions`，沿用 Bearer 登入、版本 Header、持久任務及 REST 結果。

```json
{
  "model": "quality",
  "messages": [
    {"role":"system","content":"桌面提供的技能、固定工具與單一 JSON 回覆契約"},
    {"role":"user","content":"使用者任務"}
  ],
  "stream": false,
  "execution_mode": "background",
  "conversation_id": "server_conversation_id",
  "client_request_id": "unique_request_id",
  "attachment_tokens": [],
  "skills": false
}
```

| 用途 | skills | 模型／模式 |
| --- | --- | --- |
| 一般聊天／一般附件 | true | 使用者選擇，保留串流及背景 |
| Outlook 單封／批次初篩 | false | 勾選自動補充時 quality；未勾選使用者選擇 |
| Outlook 附件最終整理 | true | quality、背景優先 |
| 標題（含專案） | false | fast、獨立對話與背景任務 |
| 專案文件每輪請求 | false | 使用者選定模型，背景 |

`skills:false` 禁止網站自動附加技能、提示詞或工具編排；保留桌面 messages，登入、模型權限及任務保存照常。不要依 `# Outlook` 等文字分流。JSON 回覆不得另行改寫為一般自然語言或強制報告格式。

桌面內部仍保留用途，對外不再傳 outlook_triage／auto_generate_title。後端需先支援 skills；本機無後端版本旗標可驗證，不宣稱已確認內網部署。舊版桌面及已持久化舊請求需要後端自行安排相容，不改寫結果未知的舊請求後重送。

## 專案工具往返

採 Outlook 類型的文字 JSON，第一版不使用原生 tool_calls。每輪只允許單一完整 JSON，不從自然語言或多份 JSON 猜指令：

```json
{"action":"tool","operation_id":"op_001","request":{"tool":"read_file","path":"report.txt","offset":0}}
```

桌面執行後，將 assistant 原文及含 operation_id 的工具結果加入 messages，以新的 client_request_id 再呼叫相同對話。連線不明時查原 ID，不换 ID 重送。每個本機操作 ID 綁定參數與結果，避免重複修改。

```json
{"action":"finish","message":"已完成修訂。","artifacts":["copy_id"]}
```

完成由 AI 提出，程式確認所有工作副本已儲存或捨棄、指定成果版本一致、重新讀回內容符合後交付。純閱讀無工作副本時可用空 artifacts。

```json
{"action":"ask_user","message":"請以記事本確認內容並匯入文字。"}
```

技能來源為 `src/projects/skill.md`。工具包含 list_files、read_file、find_text、create_working_copy、edit_text、save_copy、delete_copy。工具名稱不能擴大權限；不載入工作區中的腳本、Hooks 或其他 SKILL.md。

## 程序與文件邊界

- 使用者送出專案任務才建立 AppContainer，未授予網路能力，建立後以實際 TokenIsAppContainer 握手確認。隔離失敗停止，不退回一般權限。
- 子程序只處理固定文字操作；Windows Job 限制一個程序、128 MB，父程序關閉時終止。stdin/stdout 使用明確 handle 白名單；只傳必要 Windows 目錄環境變數，不傳登入資料或自訂環境。
- 主程序 broker 持有專案授權，逐層開啟且禁止目錄刪除／重新命名，拒絕重新解析點、硬連結、裝置名稱、ADS 與路徑越界。AppContainer 並非整個 LM_AI 主程序的沙箱。
- 工作副本先在記憶體修改，發布用 create_new，永不覆寫原檔或舊成果。delete_copy 只捨棄未發布工作副本，不刪磁碟上已發布成果。
- 每次任務最多 20 輪，連續三個工具失敗停止，30 分鐘總上限；單檔 200 KB，分段讀取最多 6000 字，最多 20 份工作副本。目錄列舉最多掃描 500 個項目、回傳 200 個；明確標示截斷。
- 編碼有 BOM 時先辨识 UTF-8／UTF-16；否則 UTF-8、CP950、系統 ANSI，拒絕替代字元與有歧義的結果。原編碼不能保存新字元時回報錯誤。匯入明文及新文件使用 UTF-8 BOM。
- 輸入文字、任務請求及操作紀錄以 DPAPI 保存於本機；不把記錄寫為工作區內的 MD。重啟時恢復完成结果或顯示中斷說明；退出不恢復本機操作授權，不自動重播舊工具。未發布的記憶體副本不跨重啟保留。伺服器請求可能仍會完成。
- 本機 smoke 只能驗證 Windows 與普通測試檔，不能替代公司加密軟體、IT 政策、GLM／Gemma 及網站端的實測。

## 更新行為

設定齒輪旁的下載箭頭只在有新版時出現，使用明顯底色，下載中停用重複點擊。下載前一次原生確認明確說明暫時關閉與重新啟動 LM_AI；同意只綁當次啟動及指定版本。

既有簽章、平台、長度、SHA-256 驗證全部保留。驗證後若為同意版本的 NSIS，保存草稿並啟動既有 `/UPDATEPID`、`/RESTART` 流程，不第二次詢問。同版本清單優先 NSIS，較新版本優先於封裝類型。僅 EXE 時提示手動更換，不假裝可自動安裝。下載／驗證失敗不退出；版本改變需重新同意；退出不觸發安裝。

本次不重建或執行 NSIS 安装驗收，待公司文件功能測試後再封裝。
