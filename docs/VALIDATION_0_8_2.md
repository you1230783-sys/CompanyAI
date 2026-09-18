# 0.8.2 連線診斷驗證

本次交付 LM_AI.exe、配對的 EXE 簽署清單與原始碼；沿用 0.8.1 的 EXE／NSIS 選版規則，不製作安裝包或離線 ZIP。儲存庫的 0.8.0 安裝包與 ZIP 是歷史產物，不代表本版。

目的為定位公司電腦的 Windows 網路錯誤 10022，尚未重現實機故障，不能宣稱連線問題已修復。使用者已確認同一電腦的瀏覽器可看到 version JSON，但此觀察不能代替 WinHTTP 或登入 POST 驗收。

本機建置結果以相同版本的 `offline/exe-verification.json` 與 `offline/environment.txt` 為準，必須完成以下檢查才可發布：

- MSVC v142 x64 編譯器探針、fmt、Clippy、工作區測試、空 Cargo 快取與專案 vendor 的 release `--frozen` 編譯。
- 新增真實 WinHTTP 無效逾時測試：回報 87 與 WinHttpSetTimeouts，未送出網路連線，錯誤文字不含測試憑證。
- 既有登入／Token、串流、更新選版與簽章等回歸測試，以及 WebView2 DOM 自我檢查。
- 重建簽署 JSON，再由本版 EXE 核對簽章、版本、長度與 SHA256。

2026-09-18 本機結果：上述檢查通過，69 項測試成功；MSVC 14.29.30133，實際 `_MSC_FULL_VER=192930159`、x64。使用空 Cargo 快取與 `--frozen`，WebView2 DOM 自我檢查及本次 EXE 清單驗證均通過。未進行公司網路、Trend 政策或實際登入故障電腦驗收。

公司驗收：從系統托盤離開舊程式，更換成 0.8.2，按瀏覽器登入。若失敗，記錄完整用途前綴、Windows 錯誤碼與括號內 API 名稱；若為 ShellExecuteW，則記錄其回傳碼。不要傳送登入碼或 Token。依實際階段再檢查公司代理、端點存取或瀏覽器關聯，不能預先歸因於 Trend。
