# 修改手冊：0.9.18 Excel 閱讀精簡（X1、X2）

撰寫日期：2026-11-06。本手冊與 CHANGE_HANDBOOK_EIS_FEEDBACK.md、CHANGE_HANDBOOK_0_9_14_TO_0_9_17.md 分開，**請勿改寫任何一份手冊**；進度與驗證結果記在 IMPLEMENTATION_PLAN.md 與 VALIDATION_0_9_18.md。

行號以提交 `2450d3f` 為準，僅供定位，請以函式名稱為主。

## 0. 背景

- 公司實測：閱讀 Excel 時，上下文很快接近 50K 輸入軟目標，接著反覆整理、重讀而卡住。
- 程式已有「匯出 CSV、模型只看摘要」的流程（`export_excel_dataset`、`plan_excel_analysis` → `export_planned_excel`、`inspect_dataset`、`run_python(kind=csv)`），但模型常改用 `read_excel_range` 逐批讀取。
- 原因有兩個：
  1. **輸出冗長**：`read_excel_range` 每格輸出完整的 `Cell` 物件（`value`、`text`、`kind`、`number_format`，以及可能的 `formula`），一格約 70 字元，真正的資料往往只有幾個字元。粗估預設 100 列 × 9 欄約 3 萬 tokens，2000 格上限約 7 萬 tokens（以 0.48 tokens／字元估算，實際以 API 用量為準）。
  2. **引導不足**：`export_excel_dataset` 的說明稱它為「舊入口」；`plan_excel_analysis` 是為畫圖設計的（需指定 X／Y／時間）。非畫圖的一般分析，模型找不到明確的匯出路徑。
- 本版只做 X1（說明文字）與 X2（精簡輸出）。B（讀取預算提示）與 C（Excel 索引工具）等公司實測後再決定，見第 5 節。

## 1. 本版範圍與驗證方式（使用者指示）

- 版本：0.9.18。一併納入 0.9.17 發布後、尚未編譯的兩項補強：M7 混合 stdout JSON 提示（`src/projects/files/python.rs`），以及 Office／CAD 鎖禁止巢狀擴充（`src/resource_locks.rs`）。
- **跳過完整驗證，只驗證相關功能**（見第 4 節）。目的是盡快產出可在公司端實測的安裝包。
- 驗證報告必須明列「本版未執行的完整驗證項目」，不得寫成完整驗收通過。
- 是否推送 Git、發布 Release，依使用者當次指示。
- 不變條件照舊：安裝路徑、公司 HTTP、更新同源與簽章、machines.json、原件唯讀、不重播成功寫入、不給 AI 刪檔能力。

---

## 2. X1：讓「一般分析」有明確的匯出路徑（只改說明文字）

### 目標
模型需要分析大量 Excel 資料、但不畫圖時，知道要「匯出 CSV → run_python 分析」，而不是用 `read_excel_range` 逐批讀取。

### 修改處與內容
| 位置 | 修改 |
|---|---|
| `src/projects/tools.json` 的 `export_excel_dataset`（約 3077 行） | 移除「舊入口」字眼。改寫為：「非畫圖的一般分析用此工具匯出選定欄為本機 CSV，再用 run_python(kind=csv) 分析；畫圖請用 plan_excel_analysis／export_planned_excel。」**保留**「已建立規劃的來源必須沿用該規劃匯出」的限制 |
| `src/projects/tools.json` 的 `read_excel_range`（約 381 行） | 明寫「只用於查看少量樣本或確認格式；大量資料請先 export_excel_dataset 再用 Python 分析，不要依 next_row 逐批讀完整份資料」 |
| `src/projects/skills/excel-read.md` | 第 1 點與最後一段：「一般閱讀依 next_row 續讀」改為上述原則；加入一般分析的流程（inspect_excel → export_excel_dataset → run_python） |
| `src/projects/skill.md` 第 49 行附近（Excel 大量資料段落） | 同步修改，刪除「依 next_row 續讀」的建議 |
| `src/projects/skills/core.rs` 第 8 行（excel-read 摘要） | 同步：「一般閱讀 read_excel_range」改成「少量樣本用 read_excel_range；大量分析先匯出 CSV」 |
| `src/projects/skills/chart-details.md` | 確認與上述說法一致，不互相矛盾 |

### 注意
- 只改說明，不改工具行為、限制或權限。
- 技能與工具說明的用語要通用，不寫成只適用於某一種資料（例如不寫 EIS 專用名詞）。

---

## 3. X2：精簡 `read_excel_range` 的輸出

### 原則
- **內部資料結構不變**：`src/projects/office/excel.rs` 的 `Page`／`Row`／`Cell`（約 100–135 行）仍由 `datasets.rs`、`excel_plan.rs`、`charts/transform.rs` 等直接使用，**不要修改它們的欄位或序列化格式**。
- **只改交給模型的工具結果**：在 `src/projects/files/dispatch.rs` 的 `Tool::ReadExcelRange` 分支（約 361–385 行），把 `Page` 轉成精簡格式後再回傳。建議新增獨立的轉換函式（例如 `office::excel::compact(&Page) -> Value`），並附單元測試。
- **資訊不能遺失**：精簡後必須能還原每一格的值、型別、數字格式、顯示文字與公式；差別只在「相同的資訊只寫一次」。

### 建議格式（可調整欄位名稱，但需滿足下方規則）
```json
{
  "format": "excel-compact-v1",
  "sheet": 1, "sheet_name": "Data",
  "columns": ["B", "D"],
  "header_row": 1,
  "headers": ["紀錄時間", "Mean值"],
  "column_kinds": {"B": "number", "D": "number"},
  "column_formats": {"B": "yyyy/m/d h:mm", "D": "0.00"},
  "row_fields": ["row", "B", "D"],
  "rows": [[2, 45964.333, 12.34], [3, 45964.334, null]],
  "texts": {"B": "per_cell"},
  "cell_texts": [{"row": 2, "column": "B", "text": "2026/11/3 08:00"}],
  "exceptions": [{"row": 15, "column": "D", "kind": "error", "text": "#DIV/0!", "number_format": "General"}],
  "formulas": [{"row": 20, "column": "D", "formula": "=AVERAGE(D2:D19)"}],
  "start_row": 2, "row_count": 2, "next_row": 4,
  "used_range": {...}, "date_1904": false,
  "path": "...", "revision": "excel:..."
}
```

### 規則
1. **列**：每列為陣列，第一個元素是原始列號，其後依 `columns` 順序放 `value`（Value2 原值）。空白格為 `null`。
2. **型別與數字格式**：每欄取最常見的 `kind` 與 `number_format` 寫入 `column_kinds`／`column_formats`；與欄層級不同的格子列入 `exceptions`。
3. **顯示文字**：`text` 與值的一般顯示相同時省略。判斷規則要寫明且可測試，例如：字串值 `text == value`；數值能把 `text` 解析成同一個數字；布林為 `TRUE`／`FALSE`。不同時列入 `cell_texts` 或 `exceptions`。日期、時間、自訂格式、錯誤值通常會被列出，這是正確結果，不要為了變短而省略。
4. **公式**：只在有公式的格子列入 `formulas`。
5. **表頭**：`headers` 用字串陣列；表頭格若有公式、錯誤或特殊格式，另列於 `exceptions`（列號為 `header_row`）。
6. **其他欄位**保留現有語意：`start_row`、`row_count`、`next_row`、`used_range`、`date_1904`、`path`、`revision`、`scope` 等。
7. **讀取上限不變**：預設 100 列、最多 2000 格／批，本版不調整（是否放寬等 B 的實測結果）。

### 必須同步修改的讀取端
新格式會影響以下解析 `read_excel_range` 結果 JSON 的程式。**每一處都要同時支援新格式與舊格式**，因為 checkpoint、封存結果與舊對話中仍保存著舊格式：

| 位置 | 用到的欄位 | 修改 |
|---|---|---|
| `src/projects/reading.rs` 的 `observe_excel`（約 69–115 行） | `headers` 陣列、`rows[].row`（首尾列號） | 新格式從 `rows[i][0]` 取列號 |
| `src/projects/files/diagram_sources.rs`（約 85–100 行） | `headers`、`rows`（500 字摘錄） | 新格式直接摘錄即可，確認摘錄仍可讀 |
| `src/projects/files/analysis.rs` 的 `record_analysis` 證據、`src/projects/analysis.rs` 的 `select` | JSON Pointer 指向結果 | 新格式的 pointer 例如 `/rows/0/2`；技能說明 `analysis-quality.md` 中 `/rows/0` 的例子要確認仍正確 |
| `src/projects/workflows.rs`（約 66 行）與工作流程步驟引用 | 已保存流程可能以 pointer 引用前一步結果 | 確認舊流程範本的引用不因格式改變而失效；若會失效，需明確檢查並提示重新保存，不可默默得到錯誤值 |
| `src/projects/overview.rs`（約 130 行）、上下文封存與 `read_work_log` 回讀 | 結果原文 | 確認新舊格式都能顯示與回讀 |

可用 `format` 欄位判斷：有 `"format":"excel-compact-v1"` 為新格式，沒有則為舊格式。

---

## 4. 本版驗證（只驗證相關功能）

### 必做
1. **建置**：v142 x64 實際編譯器核對、`--frozen`（可不用空快取）、fmt、與變更相關的 Clippy。
2. **X2 單元測試**：
   - 精簡格式可完整還原每一格的 value／text／kind／number_format／formula（以舊格式產生 `Page`，轉換後再還原比對）。
   - 數值、字串、布林、空白、日期序號、自訂格式、錯誤值、公式、不連續欄（例如 `["A","F"]`）、表頭特殊格。
   - **輸出大小**：以 100 列 × 9 欄的一般數值資料，新格式字元數應明顯小於舊格式（目標 40% 以下），並記錄實際比例。
3. **讀取端相容測試**：`observe_excel`、`diagram_sources`、`record_analysis` 證據，各測新格式與舊格式。
4. **工具定義與技能目錄測試**：0.9.12 曾因新工具遺漏共用 schema 欄位而失敗，請執行工具 schema／目錄解析的相關測試。
5. **0.9.17 補強的測試**：`summary_guidance`（混合 stdout）、`resource_locks` 新增的兩個測試。
6. **原生 Excel 測試**：實際 Excel COM 讀取固定範例（含日期、公式、錯誤值），確認工具結果為新格式且內容正確；並執行 Office 讀寫相關的原生回歸，確認新的鎖規則沒有誤擋（錯誤訊息「不可巢狀新增 Office／CAD 資源鎖」不得出現）。
7. **封裝**：產出 EXE 與 NSIS 安裝包，並做一次基本安裝、啟動、解除安裝確認。

### 本版可跳過（須在驗證報告中列出）
CAD 圖面整體驗證、Outlook、VNC、完整前端套件、完整非相關 Rust 回歸、空快取建置、完整安裝生命週期矩陣、同提交 CI（除非使用者要求推送）。

### 公司端實測要觀察（由使用者進行）
- 同一份 Excel、同樣的需求，每輪輸入 tokens 是否明顯下降，是否仍撞到 50K。
- 模型是否改走「匯出 CSV → Python」的流程。
- `read_excel_range` 的精簡格式是否讓模型誤讀資料，例如日期、錯誤值、欄位對應。

---

## 5. 待實測結果後再決定（本版不實作）

### B：同一張工作表的讀取預算提示
同一工作表累積讀取超過一定量（例如 3 批或 3000 格）時，在工具結果附上提示：「大量資料請用 export_excel_dataset 匯出 CSV，再用 run_python 分析」。只提示，不強制擋下（與 M7 的提示做法相同）。

### C：Excel 索引工具（類似 CAD 索引）
新工具（例如 `index_workbook`）：由 Excel COM 一次將各工作表的使用範圍匯出成本機 CSV（設上限），只回傳每欄概況（表頭、型別分布、空白數、最小值／最大值、前幾個不同值、公式格數）。之後模型以 `run_python(kind=csv)` 查詢。

### 決定依據
- X1、X2 已讓 Excel 任務不再卡住 → B、C 可暫緩。
- 模型仍逐批讀取 → 先做 B。
- 模型需要頻繁理解多張大表的整體結構 → 再做 C。
