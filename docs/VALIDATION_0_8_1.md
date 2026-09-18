# 0.8.1 驗證範圍

本次只交付 LM_AI.exe、對應 EXE 更新清單、原始碼與文件。安裝包及離線 ZIP 不重製；既有 0.8.0 包與其驗證記錄只代表歷史發行，不代表 0.8.1。

本機結果以本次 `offline/exe-verification.json` 與 `offline/environment.txt` 為準。驗證包括：

- 實際 MSVC v142 x64 探針、fmt、Clippy、工作區測試、release `--frozen` 編譯；指定空 Cargo 快取時從專案 vendor 取依賴。
- 以兩份既有公開簽署清單測試 EXE／NSIS 驗證、同版 EXE 優先、來源順序不影響結果、偽造高版本不能遮蔽合法資料。
- 數字版本排序優先於格式偏好；不同来源或修改 kind 後的清單拒絕。
- 本機 HTTP 模擬 download 回 404、兩份靜態 JSON 共存；仍選合法 EXE，且沒有附帶 Cookie／Authorization 或下載二進位內容。
- JSON 數量／大小限制、逐份解析、檔案大小／SHA256 錯誤拒絕、EXE 不能進入 NSIS 執行入口。
- WebView2 DOM 自我檢查：NSIS 的第二次動作、EXE 的開啟資料夾按鈕、下載後仍維持強制版本鎖定。
- 發行腳本生成實際 EXE JSON，再由本次主程式以內建公鑰核對版本、簽章、大小與 hash。

仍需公司端實測：0.8.1 首次手動部署、真實下載端點、以較高版本 EXE 手動更換與資料保留、較新 NSIS 接手、公司 Trend 防毒政策。此版不宣稱已修復先前安裝目錄的防毒攔截。
