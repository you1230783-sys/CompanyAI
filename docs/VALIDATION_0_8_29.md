# 0.8.29 原生工具與 Excel 選欄：實作及驗證

日期：2026-10-01。對應 [雙方契約](DESKTOP_AGENT_V1_CONTRACT.md)。本次依使用者要求發布 0.8.29 EXE、原始碼與簽署更新清單；不製作 NSIS／ZIP。

## 已實作的行為

- 新專案任務先查代理 capabilities，再送 `/lm_server/api/desktop/agent/turns`。缺少必要能力或 capability revision 被拒絕時停止，不自動退回文字工具協定。
- 工具定義使用原生 `tools`，每輪 `tool_choice: required`、`parallel_tool_calls: false`。31 個工具共用同一份目錄（含本次新增的 3 個 Excel 工具），轉為 `lmai-json-schema-v1`；嚴格模式依能力回應啟用，本機參數、授權、版本及成果檢查繼續保留。
- 回覆只從完整 completion 的 `choices[0].message.tool_calls` 取得操作。正文用於進度說明，不從正文抽取 JSON 執行。工具結果使用 `role: tool` 與對應 `tool_call_id`，保留成對歷史。
- `content: null` 的有效工具回覆可執行；截斷回覆不執行候選工具。拒答、多呼叫、未知工具、回應身分錯誤或嚴格 Schema 違約會停止。非嚴格參數錯誤可在既有限次修復內回饋，不能任意猜測參數。
- 每次請求保存協定、能力快照、完整請求、上下文及回覆。`turn_index` 由主模型與委派共用；本機操作 ID 由帳號、run、request、call 組合產生，允許不同輪重用同一 call ID。
- POST 結果不明及手動續接只查原請求；不以新 ID 重送。取消會對已知或查回的原 task 提出取消。既有 DPAPI checkpoint、工作副本與成功操作去重機制保留。
- 快速摘要同樣使用原生代理入口，但採 `tools: []`、`tool_choice: none` 和文字結果，沒有本機工具權限；獨立保存委派能力與待查請求。此版未要求快速模型使用可選的 `json_schema` 回覆模式。
- 一般聊天、Outlook、自動標題及 PDF 轉換維持原路由。先前版本已暫存的舊專案任務按原協定續接；這是 checkpoint 相容，不是新任務的降級路徑。

主要程式位於 `src/projects/agent.rs`、`src/projects/agent/schema.rs`，並接入既有 runner、progress、model 與 delegation。原生提示詞在 `src/projects/agent/skill.md`；舊提示詞保留供舊 checkpoint 使用。

## 驗證

使用 `scripts/Build.ps1 -EmptyCargoCache -TestOffice`，以專案 vendor、Cargo.lock 及 `.cargo/config.toml` 驗證離線 `--frozen` 建置。本次進版後重新執行完整 EXE 發行驗證，程序以退出碼 0 完成，並更新發行檔及簽署清單。

- 工具鏈：Rust 1.98.1、MSVC 14.29.30133、Windows SDK 10.0.19041.0；實際 C 探針確認 `_MSC_FULL_VER=192930159`、x64。
- 格式、Clippy（warnings 視為錯誤）、176 項單元測試及 release `--frozen` 建置通過。
- 11 個原生 HTTP 情境通過：正常讀取／副本／修改／儲存、非 strict 參數修復、POST 回覆遺失續接、截斷不執行、能力不足不降級、快速模型委派、錯誤身分、多工具拒絕、子請求失聯續接、409 明確拒絕、取消查回。每輪刻意重用 call ID，確認只修改及發布一次。
- 真實 AppContainer 的檔案／網路隔離、工作副本保護、既有 HTTP 工具流程、60 工具暫停後 DPAPI 續接、兩小時期限的短時鐘邊界模擬、筆記及 PDF 快取整合通過。期限測試不代表實際等待兩小時。
- 真實 Office COM 驗證通過（Office 16.0.20326.20158）：DOC/DOCX/DOCM、XLS/XLSX/XLSM/XLSB、PPT/PPTX/PPTM 讀取／副本修改／另存／重開，以及新建文件、結構、格式、批次成功／失敗回復。
- 新增大表 XLS／XLSX 驗證通過：超過 2000 格的文件只取 A／F、預設 100 列分批、末批與 next_row、非第一列的表頭與表頭分頁、Excel 原生公式數值畫圖、缺值保留、超量／重複欄／錯誤值拒絕、越界與 .lmai 路徑阻擋、來源變更拒絕及原檔 bytes 不變。
- 0.8.29 EXE 與更新清單已產生，透過實際 EXE 內建公鑰驗證清單簽章通過；未製作 NSIS／ZIP。機器驗證紀錄見 [EXE 驗證](../offline/exe-verification.json) 與 [Office 驗證](../offline/office-verification.json)。

發行 EXE：`dist/LM_AI.exe`，檔案版本 `0.8.29.0`，大小 `11,501,056` bytes。

SHA256：`66d232726bae04251bd704463352d61355436e12dd4dafb8b50752c55ca64636`。

## 尚待公司聯合驗收

本機 HTTP 模擬伺服器驗證桌面請求與回覆處理，不能代替實際網站與模型端測試。仍需確認網站部署新路由、宣告真實模型能力、完整保存原生 completion／messages，以及 GLM／Gemma 的原生工具與嚴格結構輸出支援。

後端維護者已回報 Nginx 處理部署前綴，並完成能力／revision、原生訊息保留、client_snapshot、帳號隔離去重及本文衝突拒絕；依回報，介面設計已對齊。本次未取得實際內網回應或執行公司模型聯測，因此不將該回報記為桌面端到端驗收通過。啟用主代理需要 native_tool_calls、required 及 lmai-json-schema-v1；strict 能力依模型實測宣告，不能以生成後驗證代替約束解碼。

公司加密文件及使用者環境、模型答案品質、Office 視覺排版不在本次本機驗證的保證範圍。

## 本次追加：Excel 大量資料選欄

使用者後續要求同版納入，新增三個桌面工具，不新增網站路由／資料表欄位：

| 工具 | 行為 |
| --- | --- |
| inspect_excel | 取得工作表清單、所選表的 UsedRange 邊界、指定表頭列與原檔版本。預設第 1 張表、第 1 列、A 起 50 欄；表頭最多 100 欄／次，可依 next_header_column 繼續。工作表名稱清單最多 100 張，另外回傳總數及截斷標記。 |
| read_excel_range | 明確指定 sheet、columns、start_row；預設讀 100 列。可只取 A／F 而不取 B–E，回傳原列號、型別、原生值、顯示文字、公式及 next_row。每批資料最多 2000 格、1000 列、100 欄，內容最多 200 KB，過大時要求縮小，不偷偷截斷。 |
| chart_excel_range | 直接從已核對版本的 Excel 取 X／Y 欄位畫圖，保留缺值及行序；使用 Excel 提供的公式數值，拒絕錯誤、文字數值與合併格。單張最多 1000 筆、8 個系列，X 加 Y 合計仍限 2000 格。來源標示實際欄位及列範圍。 |

三者使用已儲存專案相對路徑，沿用 Excel COM 唯讀開啟、禁用巨集／事件及不更新外部連結的既有設定。原檔與目錄在操作期間鎖定；`excel:` 版本來自原檔 bytes 指紋，不能混用 read_file 的文字快照 revision。版本變更需重新取表頭。原生日期 value 為序號，text 為顯示文字，另外回傳 Date1904 設定。[Microsoft Value2 說明](https://learn.microsoft.com/en-us/office/vba/api/excel.range.value2)。

範例：inspect_excel(path="資料.xlsx") → read_excel_range(path, revision, sheet=1, columns=["A","F"], start_row=2)，預設得到 100 列／200 個資料格；下一批依 next_row（例如 102）繼續。表頭非第一列時指定 header_row。

這是局部取值，不登記為已讀全文，也不觸發全文件摘要要求。進度摘要不重複攜帶整批 rows／headers；真實工具歷史及操作紀錄仍可查回。不得把前 100 列冒稱為全部趨勢，不自動抽樣；超出單張上限時明確分圖或詢問範圍。

新工具不再因整份 UsedRange 超過 2000 格而拒絕讀取；Office 完整編輯快照及大型文件修改仍維持原限制。未儲存的小型副本沿用 read_file／chart_from_excel；如需新選欄工具，先 save_copy 再使用已儲存路徑。這次未加入任意公式計算、篩選查詢或自動彙總。
