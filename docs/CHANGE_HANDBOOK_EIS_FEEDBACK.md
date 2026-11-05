# 修改手冊：EIS／DRT 實測回饋（0.9.11 之後）

撰寫日期：2026-11-05。依據：使用者以 0.9.11 對模擬 EIS CSV 執行 DRT 繪圖與分析的完整專案紀錄（21 輪、716,566 tokens、42 分 29 秒），以及 Claude 對程式碼的檢查。行號以提交 `2cd2976` 為準，僅供定位；請以函式名稱為主。

## 0. 先讀：背景與已確認事實

- 本次 8 次 `finish_reason: "length"` 的回覆，`content` 皆為空字串、沒有工具呼叫，`completion_tokens` 剛好 20,000。成功輪次的可見輸出遠小於 completion_tokens（例如只送 `update_deliverables` 的那一輪用了 17,210）。**結論：輸出額度被模型隱藏推理耗盡，不是報告或程式碼太長。** 服務未回傳 `reasoning_tokens`。
- 相同輸入可能一次超過 20K、一次只用 8.5K（第 18、19 輪輸入同為 25,696），自動續接成功與否接近隨機。
- 伺服器輸出上限已由使用者在 lm_server 調為 32K（已觀察到約 24K 的成功輸出）。模型上下文長度 1M，**不需要**修改 50K 輸入預算。
- 現有程式**已具備** checkpoint、操作 ID 去重、`finish_reason` 截斷偵測、續接提示、`no_progress` 計數，以及 `cached_tokens`／`reasoning_tokens` 欄位記錄（`src/projects/context.rs` 的 `actual`）。**不要另建 checkpoint 或事件日誌系統。**

## 1. 不變條件（每項修改都要遵守）

- 沿用 docs/IMPLEMENTATION_PLAN.md 的不變條件：安裝路徑、公司 HTTP、更新同源與簽章、machines.json、原件唯讀、不重播成功寫入。
- **不得給 AI 刪除檔案的能力**。被取代的成果保留在磁碟上，只改變標示與交付清單。
- 不以模型自述代替工具紀錄；新增的檢查以原生紀錄為準。
- 每項獨立 commit，commit 訊息列出修改處、測試與限制；未實測項目不得標示通過。

## 2. 修改項目（依優先順序）

### M1. 參數被規則拒絕時，不要顯示 Error Code 99

- **現象**：第 5 輪 `record_reading_notes`、第 15 輪 `update_deliverables` 被防護規則正常拒絕，畫面卻顯示「Error Code: 99 操作未完成，原因尚無法確定。請保留畫面提示並匯出錯誤資訊，交由維護者確認」。AI 下一輪已自行修正，但使用者會以為程式故障。
- **原因**：工具回傳 `ok:false` 時一律走 `crate::errors::report`，而 `classify` 以字串比對分類，這類契約拒絕沒有對應代碼，因此落入 99。
- **目標**：區分「參數或契約被拒絕、未執行任何操作」與「執行中失敗或結果不明」。前者顯示新代碼，例如 `14`：「AI 的操作參數不符規則，已拒絕且未執行；AI 會依原因自行修正」，建議文字不要求使用者匯出診斷。
- **修改處**：
  - `src/projects/runner/execution.rs`，工具失敗分支（約 1060–1075 行，`result["ok"] == false`）。
  - `src/errors.rs` 的 `classify`（約 74 行）與 `description`（約 55–70 行）。
  - `src/ui/diagnostics.rs` 的 `export_diagnostics` 寫死的代碼清單 `"01"…"13","99"`，需加入新代碼。
  - `docs/ERROR_CODES.md` 同步。
- **做法建議**：由 Broker 在拒絕結果中帶結構化欄位（例如 `"rejected": true` 或 `"error_kind": "contract"`，並保證 `executed:false`），分類依欄位判斷，不新增脆弱的字串比對。已知拒絕來源至少包含 `src/projects/deliverables.rs`（約 64 行）與 `src/projects/reading.rs`（約 366 行）。
- **驗收**：上述兩種拒絕顯示新代碼與正確說明；真正的執行失敗或未知結果仍維持原代碼；診斷匯出包含新代碼說明。

### M2. 交付清單：只綁定路徑，並支援「已取代」

- **現象**：
  1. 第 15 輪 AI 只想替既有項目補上檔案路徑，卻必須重送整份清單，描述文字改了幾個字就被拒絕。第 19 輪逐字重送才成功，兩輪合計浪費約 25K tokens 與數分鐘。
  2. 最終「成果檔案」清單列出 v1 檔案（`DRT_結果.csv`、`DRT_圖譜.png`、`DRT_Lambda掃描.png`），但報告寫著 v1 請勿使用。
- **原因**：
  1. `src/projects/deliverables.rs` 的更新邏輯（約 40–75 行）在條件版本未變時，禁止修改既有項目的 description，且只有整份覆寫的入口。
  2. `src/projects/files/completion.rs` 的 `finish_cancellable` 會把所有 datasets、python_artifacts 等成果全部加入 `paths`（約 56–80 行），沒有參考交付清單或取代關係；`src/projects/runner/execution.rs` 約 1176 行再把它們全部列為「成果檔案」。
- **目標**：
  1. 新增輕量工具，例如 `bind_deliverable { id, target }`：只更新指定項目的 target，其他欄位不動、不需重送描述。
  2. 新增取代標記，例如 `supersede_artifact { path, replaced_by, reason }`，或在交付項目中記錄被取代的路徑。**只改標示，不刪檔。**
  3. 最終成果清單優先列出交付清單綁定的檔案；被取代者另列「已取代（保留未刪除）」區塊，或不列出。
  4. `update_deliverables` 拒絕修改描述時，錯誤訊息提示「只需綁定路徑請改用 bind_deliverable」。
- **修改處**：`src/projects/deliverables.rs`、`src/projects/tools.json`（工具定義）、`src/projects/files/dispatch.rs`（工具分派）、`src/projects/files/completion.rs`、`src/projects/runner/execution.rs` 的 Finish 分支，以及相關技能說明。
- **驗收**：重現本案，AI 可以一次呼叫就綁定 6 個檔案；v1 檔案仍在 `_AI_Output`，但不出現在主要成果清單中；交付核對（存在性、雜湊）不變。

### M3. 用 Python 讀取原文時，閱讀筆記的處理

- **現象**：第 4 輪 AI 用 `run_python(kind=text)` 讀 CSV 原文；第 5 輪想用 `record_reading_notes` 保存資料事實被拒絕（「筆記只能引用已成功交付的閱讀操作」）。事實只能寫進工作筆記，第 6 輪壓縮後流失，**最終報告的頻率範圍因此寫錯**（見 M5）。
- **原因**：`src/projects/files.rs` 的 `observe_reading`（約 1108–1127 行）只把 `ReadFile`、`ReadDocumentSection`、`ReadCodeSection`、`AnalyzeImage` 登記為閱讀來源；`RunPython` 不會進入 `reading.pending`。
- **目標**（建議採用 a + b，不建議讓 Python 輸出直接成為閱讀來源，因為 Python 輸出是模型程式產生的衍生文字，不是原文區段）：
  - a. python-analysis 技能說明明確寫出：讀原文、確認格式要用 `read_file`；`run_python` 用於計算。
  - b. `src/projects/reading.rs` 約 366 行的錯誤訊息，在 operation_id 屬於 Python 操作時，改成具體指引：「Python 結果不是閱讀來源；原文請用 read_file 閱讀後再記錄，計算結果請用 register_report_values 登記（見 M4）」。
- **驗收**：同樣情境下，AI 改用 read_file 閱讀，或依指引登記計算值；不再出現同一種拒絕。

### M4. 資料事實與關鍵數值：沿用既有的報告數值綁定

- **現象**：報告的主要數值（Rs、Rpol、各峰 R、Ceff、殘差）正確，但資料描述錯誤：
  - 報告寫「1×10⁻² ～ 1×10⁵ Hz、7 個 decade、每 decade 81 點」，實際為 1×10⁻³ ～ 1×10⁵ Hz、8 個 decade、每 decade 約 15 點。
  - 報告寫「τ 網格 [10⁻⁷, 10³] s」，DRT_結果_v2.csv 實際為 1.59×10⁻⁸ ～ 1.59×10⁴ s。
  - 報告寫「Δlog₁₀τ = 10/119 ≈ 0.084」，實際間距 0.1008（12/119）。
  - 報告寫「量測頻率對應 τ ≈ 1.6×10⁻⁶ ～ 15.9 s」，實際上限 159 s。
  - 「P4 位於量測下限一個 decade 內、面積上界受限」是由錯誤的頻率範圍推導出來的。
- **原因**：資料事實只存在已被壓縮移出的上下文中，模型撰寫報告時自行補值。
- **既有機制**：`src/projects/report_values.rs`、`src/projects/files/report_values.rs`（`register_values`，以操作 ID 綁定 Python 實際結果）與 `finish_cancellable` 內的 `verify_report_values`。**請擴充這套機制，不要另建 results.json 系統。**
- **目標**：
  1. 已登記的計算值（含資料驗證結果：筆數、欄位、頻率範圍、取樣密度、單位）在每輪請求中以固定小區塊提供，壓縮時不移出。先確認 `report_values.context()` 目前是否每輪都會帶入；若沒有，加入，並控制大小。
  2. python-analysis 技能說明要求：資料驗證與最終計算步驟，以 `register_values` 登記報告會引用的數值。
  3. 交付文字回答時，由程式在回答後附上「本次登記的計算值」表（來自登記內容，不經模型改寫），作為權威數值。
  4. 第二階段（可另行排期）：對回答中可辨識的數值做不一致偵測，發現與登記值不符時，退回一次請 AI 修正，或在回答中標示。
- **驗收**：重現本案，報告後附的權威表顯示正確的頻率範圍與點數；登記值在壓縮後仍可見；未登記數值的回答行為與現在相同。

### M5. 輸出額度耗盡的續接：偵測後提早停止

- **現象**：截斷輪次的輸入幾乎完全相同（第 10～13 輪皆為 26,036），續接提示每次只有次數不同；而且提示內容是為 CAD 撰寫的（「CAD 尺寸／引線先拆成每段 1–3 項」），與一般任務無關。
- **原因**：`src/projects/progress.rs` 的 `retry_output_limit`（約 394–415 行）使用固定文字；`src/projects/runner/execution.rs` 約 655–677 行只累計次數，不判斷截斷型態。
- **目標**：
  1. 續接提示改為通用文字；CAD 專用的拆分建議只在 CAD 任務出現。
  2. 截斷時判斷 `content` 是否為空、有無工具呼叫：可見內容為空，代表額度被推理耗盡。
  3. 連續 2 次「可見內容為空的截斷」且輸入 tokens 幾乎相同（例如差異 < 1%）時暫停，說明「模型推理超過輸出上限，不是內容太長；可按繼續重試，或調整模型推理設定」，不要用完 5 次。
  4. 若服務有回傳 `reasoning_tokens`，在說明中一併顯示。
  5. 保留現有原則：截斷內容不執行、不重播成功操作、checkpoint 照常保存。
- **驗收**：以模擬回覆（`finish_reason:"length"`、`content:""`、相同 prompt_tokens）測試第 2 次就暫停；有可見內容的截斷維持原續接流程；CAD 任務仍保有原本的拆分提示。

### M6. 用量表加入耗時與截斷資訊

- **現象**：目前的用量表只有輸入、輸出、合計，看不出哪些輪次是截斷、花了多久。本案光截斷輪次就耗掉約 25 分鐘。
- **修改處**：`src/projects/diagnostics.rs` 的 `short_usage`（約 69 行）與每輪紀錄（`turn["actual"]`）的寫入處。
- **目標**：每輪增加「等待耗時（秒）」與「結束原因」（`finish_reason`）；服務有回傳時增加「推理 tokens」與「快取 tokens」。缺值顯示「未取得」，不推算。合計列顯示截斷輪數與截斷總耗時。
- **驗收**：既有測試的合計格式維持；新增欄位在缺值時顯示「未取得」。

## 3. 不在本次範圍

- 重建 checkpoint 或事件日誌：已有，不重做。
- 讓 AI 刪除成果檔：不做（見 M2）。
- 報告分段寫入檔案、`finish` 只交摘要：本案的報告總共約 14K，不是瓶頸，列為日後選做。
- 依上下文長度動態計算輸入預算：模型上下文長度 1M，不需要。
- 伺服器輸出上限：已由使用者在 lm_server 設為 32K，程式不需修改。

## 4. 整體驗收

1. 各項單元測試與既有回歸維持通過；新增測試對應每項驗收條件。
2. 用同一份 `EIS_模擬量測.csv` 與相同需求重跑真實模型，記錄並比較：Error 99 次數、交付清單重送次數、截斷次數與耗時、報告資料描述是否正確、成果清單是否排除 v1。
3. 真實模型的結果每次可能不同，比較時如實記錄，不只挑成功的一次。
4. 完成後交由使用者安排 Claude 複核。
