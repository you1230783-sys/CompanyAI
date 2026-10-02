# 0.8.33 圖表資料決策與按需工具

## 行為

- Y 軸（以及散佈圖 X）接受有限的十進位數字文字，可含正負號、小數、科學記號與前後空白。千分位、百分號、單位不猜測；類別 X 保留字串。
- 按欄位及類型分組列出異常，最多三個範例／組。使用者選 gap、skip 或 zero；X 異常只能明確排除整列。決策無模型可寫欄位，UI 帶 run/request 身分，過期或重複回覆拒絕。
- gap 保留 null；skip 只排除該系列的點並保留原 X 座標，真正空白仍斷線；zero 只改選定異常。圖表卡與 PNG 使用同一組資料與座標，附處理計數。來源 Excel 不修改。
- 決策期間已釋放 Excel，接受前重驗來源。可暫存，之後接續原已完成模型請求，不增加錯誤計數、不重送推論。停止／離開取消等待；共用本段兩小時期限。
- 快照工具與選欄工具共用預檢。舊快照無法取得公式數值時仍列為待處理特殊值，建議對已存檔來源用 chart_excel_range。

## 按需載入

初始基本工具為 list_files、read_file、load_skill、ask_user、finish、read_work_log、read_task_result。九項技能簡介各不超過 30 字；text-edit、office-edit、excel-read、research、notes、charts 是基本群組，paper-evidence、weekly-update、multi-file-excel 組合所需群組。

load_skill 下一輪增加對應完整 Schema 和 system 說明；工具結果不再重複全文。同一組說明只附一次，已載入集合保存到 checkpoint，舊集合恢復時補齊依賴。快速模型仍沒有委派工具，沒有副本／圖表仍沒有對應編輯／匯出工具。網站沿用 desktop-agent-v1，不新增路由或資料表。

## 驗證狀態

執行 `scripts/Build.ps1 -EmptyCargoCache -TestOffice` 完整通過：

- MSVC 14.29.30133／v142 x64，實際探針 `_MSC_FULL_VER=192930159`；Rust 1.98.1。空 Cargo 快取，既有 Cargo.lock、vendor、.cargo/config.toml，`cargo build --release --frozen` 成功。
- 格式、Clippy 禁止警告及 194 項單元測試通過；包含十進位文字／地區格式、原空白、單系列略過、X 排除對齊及舊快照空白相容。
- WebView2 DOM 自檢通過：預設 gap、使用者選擇與身分回傳、Escape 暫停、樣本文字不當 HTML，以及固定 X 座標／空白／其他系列保留。PNG 正式 callback 匯出 10000 點；已目視核對曲線、座標、標籤及 3000–3099 缺值斷線。
- 原生代理 12 種 HTTP 情境通過。新增情境先載入 charts，再讀快照、遇 NG 暫停、恢復後選 zero 並交付；總共四次 POST，續接沒有重送推論。技能回歸改為檢查載入 ID 及工具結果不重複說明。
- 既有文字協定、快速摘要委派、60 工具暫停恢復、未知請求／逾時、PDF、DPAPI 記憶與 AppContainer 隔離回歸通過。
- 真實 XLS／XLSX：數字文字 339、NG、帶單位文字、Excel 錯誤、合併格均正確分類；三種選擇、等待後同 ID 執行、重複成功不再詢問、來源在等待期間變動拒絕均通過。原空白與另一系列數值保留，來源 bytes 不變。10000 點、選欄及分頁回歸通過。
- Word／Excel／PPT 十種新舊格式，以及新建文件、格式、PNG 嵌入／另存／重開回歸通過。
- 原始技能正文由 7604 降為 1816 UTF-8 bytes（3678 → 806 字元）；實際初始 system 含九項目錄為 2562 bytes。初始原生工具七項，strict 為 3985 bytes、非 strict 為 3992 bytes。這些是文字／序列化大小，不是 token 數，也不是模型成功率測量。

建置期間先發現舊整合測試仍期待 load_skill 在工具結果附全文；更新成新契約後重新執行完整建置，最後全部通過。另保留舊 Office 快照的空白 text 為缺值並加入回歸測試。

EXE／簽署清單與建置記錄見 [EXE 驗證](../offline/exe-verification.json)、[Office 驗證](../offline/office-verification.json)。使用本機固定資料，不代替公司加密或 GLM／Gemma 真實品質測試。不製作 NSIS／離線 ZIP。
