# 0.8.75 全功能互動教學與獨立預覽

使用者已確認原三個專案操作練習可用，本輪擴充為 55 個獨立互動主題。
原教學主題全部轉為假資料練習，另涵蓋新功能與原本沒有教學的入口。
分類包括一般對話、專案操作、設定與通知、成果與重用、資料分析與報告、圖表與流程圖、郵件與週報、VNC 及 CAD。
完整主題及步數見 [教學覆蓋表](TUTORIAL_COVERAGE.md)。

## 手動查看

雙擊專案根目錄的 `Preview-Tutorials.cmd`（原 `Preview-ProjectTutorial.cmd` 仍可用），或執行：

```powershell
./target/x86_64-pc-windows-msvc/release/company-ai.exe --tutorial-preview
```

此入口使用編譯後應用程式與 WebView2，直接開啟教學選單，不需要登入。
安裝後也可在 PowerShell 執行 `& 'C:\largan\LM_AI\LM_AI.exe' --tutorial-preview`；根目錄兩個 CMD 是原始碼開發目錄的啟動捷徑。
沒有正式 App 控制器與專案命令接收端，不讀取正式帳號、對話或設定。
預覽中的完成紀錄只維持到視窗關閉；WebView2 profile 位於獨立暫存目錄。

正式介面登入後的入口為「設定 → 教學與更新日誌 → 選擇互動教學」。
首次提供選單，離開或跳過後不再自動彈出；完成個別練習才保存該主題的完成旗標。
既有版本更新不會清除其他主題的完成紀錄。選單已提供的旗標獨立保存。

## 行為與界線

- 外圍變暗、亮框標示當前操作；說明顯示在右側，窄視窗改放下方。
- 選單支援分類、關鍵字搜尋、完成數量與再次練習，回選單保留目前篩選。
- 支援鍵盤操作、上一步、回選單、離開與重新練習；框位置隨捲動與視窗大小更新。
- 範例資料、專案名稱、資料夾選擇器、檔案預覽與 AI 回覆均由前端模擬。
- 範例要求可以編輯，但示範答案固定；畫面明示不分析使用者輸入的新問題。
- 不呼叫模型、不消耗模型 tokens、不建立真專案／資料夾，不讀寫來源、郵件或 CAD。
- 正式版僅透過外層回呼保存選單旗標與完成紀錄；練習不使用正式對話與輸入框。
- 示範檔案預覽用於理解來源；不代表正式介面可手動預覽所有檔案格式。
- 所有主題使用固定範例；自訂需求不會被模型分析。圖表標題與文字樣式等指定練習會在教學內更新預覽。

## 維護教學內容

原三個練習在 `ui/project-tutorial.js`，其餘宣告式場景在 `ui/tutorial-lessons.js`。
共用呈現器只支援固定的表格、文字、對話、差異、程式、圖表、流程及 CAD 示意，
操作限定為模擬按鈕、文字輸入、選單與確認框，不接正式 App 命令。

新增或修改目錄後執行 `node scripts/Sync-TutorialCatalog.js`，同步原生 `assets/tutorials.json` 與覆蓋表。
既有文字卡轉互動時增加該主題 revision；原三個已核可教學的完成旗標沿用。

## 本機編譯

```powershell
./scripts/Build.ps1 -BuildOnly -EmptyCargoCache
```

沿用 v142 x64 實際編譯器探針、Cargo.lock／vendor／.cargo/config.toml，空 Cargo 快取下執行 `cargo build --workspace --release --frozen`。
只產生 target 候選 EXE 與 `.build/build-only.json`，不改版號、dist、更新清單、安裝包或 Git。
此模式不跑測試；編譯成功不代表教學互動與外觀已通過使用者驗收。
