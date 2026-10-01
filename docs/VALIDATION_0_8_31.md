# 0.8.31：一萬筆圖表、PNG 匯出與本機執行紀錄

依使用者最新指示，本批進版 0.8.31，驗證後發布 EXE、簽署清單、原始碼與文件；不製作 NSIS／ZIP。

## 圖表

- Chart 及工具定義上限統一為 10000 筆、8 個 Y 系列；數值大小沒有新增 ±10000 限制。保留缺值及所有選取點，不抽樣。
- chart_excel_range 使用獨立 90000 格／32 MiB 暫存額度，最多讀 X 加 8 個 Y 欄；原檔版本、目錄鎖、Excel COM 唯讀及取消檢查保留。一般 read_excel_range 仍限 2000 格／1000 列／200 KB，不把大型原始數值送回模型。
- 每張圖資料最多 8 MiB，任務合計 16 MiB，仍最多 12 張；大圖資料表展開後每頁 100 列。畫面可縮放查看，PNG 固定輸出完整選取範圍。
- export_chart_png(chart_index, name) 使用本次建圖回傳的 0 起算編號、單一 .png 檔名。固定 1600×1000 白底，包含標題、圖例、座標與來源。模型不能提供 JS／ECharts option／PNG bytes。
- 背景任務透過 UI 執行緒呼叫固定 WebView2 函式，取消或 45 秒未完成便停止等待；離屏繪圖不依賴目前聊天頁或 requestAnimationFrame。晚到的回應不寫檔。
- 圖片容器、CRC、尺寸及寫入後 bytes 核對；輸出到 _AI_Output/YYYYMMDD_HHMMSS，以 create_new 保護原檔，撞名加序號。同圖同名沿用已驗證成果，暫停續接保留路徑及雜湊；交付前再核對。
- PNG 不屬於文字工作副本；finish.artifacts 仍只填工作副本 ID，PNG 成果由程式自動併入。未取得 verified=true 不得宣稱存檔完成。

## 診斷

- 失敗狀態顯示工具錯誤與操作 ID，不只顯示「失敗」。
- 執行中及完成／失敗對話提供「執行紀錄」，從本機 DPAPI 日誌讀取，可重新讀取及複製。
- 新任務每輪另存可讀回覆摘錄、Request ID、Task ID、狀態與錯誤；工具要求／結果按請求配對。長內容明確標示截斷，顯示不包含整份系統提示詞與重複歷史。
- 舊版紀錄可顯示已保存的工具要求／結果及最後回覆；未保存的逐輪回覆不虛構還原。
- UI 核對目前對話與 run ID，本機讀取不上傳；紀錄僅按使用者操作解密，帳號 Token 不顯示。複製紀錄可能含使用者文件內容。
- 網站是否將新 agent 任務存入一般聊天資料表，不能由目前進度訊息判定；應依 Request ID 查代理持久任務及後端日誌。本批不修改網站保存流程。

## 驗證

執行 `scripts/Build.ps1 -EmptyCargoCache -TestOffice`，使用 MSVC 14.29.30133／v142 x64，實際編譯器探測為 `_MSC_FULL_VER=192930159`。

- 格式檢查、Clippy（禁止警告）、184 項 Rust 單元測試通過；以空 Cargo 快取及本地 vendor 執行 `cargo build --release --frozen` 成功。
- WebView2 DOM 自我檢查通過；實際透過原生 ExecuteScript 回呼取得 10000 點 PNG，Rust 解碼及校驗成功，並開啟影像確認中文標題、座標、來源與 3000–3099 筆缺值斷線正常。影像為 1600×1000、128559 bytes。
- `node scripts/Test-Charts.js` 通過模擬繪圖器的 10000 點、8 個系列、缺值、全範圍匯出參數及資源清理測試。JavaScript 語法檢查與 `git diff --check` 通過。
- 原生代理 11 個情境、舊協定回歸、長任務暫停／續接、PDF 與 AppContainer 隔離測試通過。
- 實際 Word／Excel／PowerPoint 新舊格式的讀取、修改、另存與重新開啟通過；新建文件、結構／格式、批次失敗回復亦通過。XLSX 及 XLS 均實測非連續 A／F 欄、100 列分頁、10000 點圖表的公式數值與缺值保留，確認來源檔案 bytes 不變。
- EXE 與簽署更新清單已產生；版本、SHA256、工具鏈與完整建置結果見 [EXE 驗證](../offline/exe-verification.json)、[Office 驗證](../offline/office-verification.json)。未製作 NSIS 或離線 ZIP。

公司加密環境、實際 GLM／Gemma 端到端任務及網站資料保存流程不在本機驗收保證範圍。
