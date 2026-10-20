# 0.8.4 估時移除與設定版面驗證

本次交付 LM_AI.exe、配對 EXE 簽署 JSON、原始碼及驗證文件。沿用使用者指定的 EXE 發行方式，不製作 NSIS 安裝包或離線 ZIP；既有包仍是歷史版本。

變更範圍：移除估時按鈕、命令、請求、資料型別及顯示；狀態說明移到模式按鈕同一列；設定面板加寬並處理內容收縮／換行，修正 radio 被通用 input 寬度及 fieldset label 舊樣式影響的排列問題。

驗證包含：

- `scripts/Build.ps1 -EmptyCargoCache`：實際 MSVC v142 x64 編譯探針、fmt、Clippy、工作區測試、空 Cargo 快取與 vendor 的 `cargo build --release --frozen`。
- 舊任務／附件 timing 欄位相容性：仍能讀取、驗證及保留實際進度／順位，重新序列化後不再保存估時。
- 實際 WebView2 DOM 自我檢查：模式按鈕與狀態同列；設定面板 320／520 px、字體 12／14／20 px 的六種組合均不應水平溢出，radio 位於文字第一行旁，選項及控制項不被水平裁切。
- 既有登入、附件、任務、串流、通知、內網直連與 EXE／NSIS 更新驗證；版本門檻及 EXE 手動更新介面仍接受檢查。
- 重建更新 JSON，由本版 EXE 驗證簽章、版本、檔案大小及 SHA256。實際建置結果與 hash 見 `offline/exe-verification.json`、`offline/environment.txt`。

2026-09-19 本機結果：上述檢查通過，72 項 Rust 測試成功，WebView2 六種排版組合及 EXE 清單驗證均成功。MSVC 14.29.30133，實際 `_MSC_FULL_VER=192930159`、x64。

版面檢查使用 WebView2 的實際幾何資料；本次尚未取得人工外觀截圖，亦未在公司電腦驗證各種 Windows 顯示縮放。公司端可直接更換 EXE 後，確認常用視窗大小、字體與單選設定操作。
