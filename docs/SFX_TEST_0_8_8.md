# 0.8.8 自解壓縮測試包

使用者回報精簡 NSIS 被防毒攔截，本輪以 WinRAR 官方標準 GUI SFX 模組製作另一種手動交付包，供實際環境比較。未關閉防毒、加入排除項目或對程式加殼／加密；沒有更改 LM_AI 主程式。

使用者後續確認防毒廠商為趨勢，提示約為「新發現的程式」，尚未取得精確偵測紀錄。這可能對應 Newly Encountered Program Protection；[趨勢官方說明](https://docs.trendmicro.com/en-us/documentation/article/apex-central-widget-and-policy-management-guide-newly-encountered-pr)指出，其依據包含檔案普及程度及歷史時間。這仍是依提示描述的推測，不能認定為病毒偵測或宣稱已排除問題；更換包裝後產生的新檔案也可能被相同政策提示。本測試包不取代公司 IT 對檔案的驗證與核准。

## 使用方式

1. 先從系統托盤選擇「離開」，完全退出 LM_AI。
2. 開啟 `dist/LM_AI_SFX_Test.exe`，閱讀說明後按「解壓縮」。預設位置為 `C:\largan\LM_AI`，使用者可在視窗改位置。
3. 若目的地已有 `LM_AI.exe`，按下解壓縮後會替換；`machines.json`、`user_config.json` 與其他檔案不在包內，不會被隨包覆蓋。
4. 解壓完成後自行開啟目的地中的 `LM_AI.exe`。若缺少 WebView2，先安裝公司另行提供的 x64 Runtime。

此包只負責解壓縮，不建立捷徑、解除安裝項目或自動啟動，不檢查 WebView2，不等待已開啟的 LM_AI 退出，也不提供 NSIS 的更新回復機制。它不是目前自動更新協定支援的格式；不得用它取代 Setup 或宣告成 `kind: nsis`。

## 製作與驗證

使用官方 WinRAR 7.23 x64 繁體中文發行包：

- 來源：<https://www.rarlab.com/rar/winrar-x64-723tc.exe>
- 官方下載頁：<https://www.rarlab.com/download.htm>
- 下載包 SHA256：`c3d2bb68848646d68fe30f7aa345bd21bfe26abed5ba597c7a575f034ccd3f0d`
- 下載包 Authenticode：Valid，簽署者 win.rar GmbH。這是工具下載包的簽章，不表示 LM_AI 的 SFX 成品具有該發行者簽章。
- 工具只解壓到 `.build/winrar-7.23-tc`，沒有安裝到系統，也不隨產品散布。WinRAR 提供 40 天試用；若日後持續用於正式製作，應使用有效授權。

`installer/LM_AI-sfx.txt` 保留可閱讀的繁中設定。`Build-Sfx.ps1` 會轉成 UTF-16 註解，核對 Rar.exe／Default.SFX 的 SHA256，使用未修改的標準 SFX 模組，並以明確的單檔清單打包，避免意外加入密碼或使用者設定。製作完成後驗證壓縮完整性及只有一個 LM_AI.exe。

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Build.ps1 -EmptyCargoCache -ValidateOnly
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Build-Sfx.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Test-Sfx.ps1
```

2026-09-22 上述流程全部通過。Rust 主程式使用 MSVC v142 14.29.30133、實際 `_MSC_FULL_VER=192930159` x64；fmt、Clippy、81 項 Rust 測試、空 Cargo 快取 frozen release 與 WebView2 自檢通過。封裝器本身使用官方預編譯模組，未重新編譯 WinRAR。

實際執行成品 SFX：未指定目的地覆寫參數，確認包內預設路徑；替換合成舊 EXE；確認 VNC 設定及未知檔案保留；核對解壓後 EXE SHA256 並執行 WebView2 自檢；確認捷徑／解除安裝登錄沒有更動。測試後只刪除本次已知測試檔與空目錄。測試會拒絕使用已存在的產品目錄，避免覆蓋真實安裝。

- 成品大小：2,095,067 bytes（約 2.10 MB）。
- SFX SHA256：`61bcf0e82567dd042c2e8c071b57fadf9aa56a5f6bad85b87b2547aa0df7daaa`。
- EXE SHA256：`095cde161bfb3331d7a9a8329c63a6995806ad01be723224ec1ee49c6071f8da`，與使用者已驗收的 0.8.8 完全相同。
- 詳細紀錄：`offline/sfx-verification.json`。

自動測試以標準 `-s` 參數隱藏對話方塊；尚未視覺驗收互動視窗，也未在使用者被攔截的防毒環境測試。不能據此宣稱防毒問題已解決。正式 NSIS、EXE、更新清單及歷史離線 ZIP 保持原狀。
