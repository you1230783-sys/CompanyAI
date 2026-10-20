# 0.8.3 隔離內網連線驗證

本次交付 LM_AI.exe、配對 EXE 簽署清單與原始碼，不製作 NSIS 安裝包或離線 ZIP。既有 0.8.0 包為歷史產物，不代表本版。

公司觀察：0.8.2 在啟動取得模型及申請登入碼時皆回報 `10022（WinHttpOpen）`；同電腦的瀏覽器可讀取 version JSON。使用者確認網路與外網完全隔離，Windows 代理採自動偵測。

本版將固定公司 origin 改為 WinHTTP 直接連線，作為修正候選；只確認問題發生在 session 初始化，尚未證實自動代理或 Trend 的具體根因。

本機必須驗證：

- MSVC v142 x64 實際編譯器探針、fmt、Clippy、所有測試與 release `cargo build --frozen`，使用空 Cargo 快取及專案 vendor。
- 完整 origin 邊界：公司固定来源、大小寫及預設埠、loopback 可直連；其他主機、相似網域、私有 IP、不同 scheme／port 不套用公司直連規則。
- 以真正 WinHttpOpen 建立公司 API／通知 session，再以 WinHttpQueryOption 核對 `NO_PROXY`；不送出公司或外網請求，不修改系統設定。
- 既有本機模擬登入、Token、串流、附件、通知及 EXE／NSIS 選版回歸測試。
- WebView2 DOM 自我檢查；生成簽署清單並由本版 EXE 核對版本、大小、SHA256 與簽章。

2026-09-19 本機結果：以上檢查通過，71 項測試成功，實際查詢到公司 API／通知 session 均為 NO_PROXY。MSVC 14.29.30133，實際 `_MSC_FULL_VER=192930159`、x64；空 Cargo 快取與 `--frozen` 建置、WebView2 DOM 自我檢查及本版 EXE 簽章驗證均成功。產物雜湊與完整紀錄見 `offline/exe-verification.json`、`offline/environment.txt`。

公司故障電腦尚待重新驗收：離開舊版，開啟 0.8.3，先看模型清單，再按瀏覽器登入。若仍失敗，回報完整步驟、錯誤碼及括號內 API；若顯示 `WinHttpOpen/direct`，代表錯誤仍在直接連線 session 初始化階段。不可因本機測試通過就宣稱公司已修復。
