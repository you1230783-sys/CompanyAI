# 離線建置與 GitHub 發行

從0.9.1起，Git保存原始碼、Cargo.toml／Cargo.lock、建置腳本、依賴版本與雜湊、必要圖片／字型／文字資源及驗證JSON。字型使用普通Git，不再需要Git LFS。

## 手動安裝與公司更新

[GitHub Releases](https://github.com/you1230783-sys/CompanyAI/releases) 提供 `LM_AI_Setup.exe`、`update-manifest.json`、`update-manifest-exe.json` 與 `SHA256SUMS.txt`。兩份JSON均指向同一NSIS成品，沿用既有相容契約；不提供獨立LM_AI.exe下載。請依Release中的SHA256及驗證紀錄核對。

GitHub只供手動下載。程式內更新繼續使用公司HTTP及 `/lm_server/desktop/releases/` 的指定檔案、RSA簽章、SHA256與同源檢查；JSON中的網址不改成GitHub。發布Release不代表已部署公司伺服器。

WebView2不再存放於Git或Release，請依[微軟下載指引](../../dist/WEBVIEW2-OFFLINE.md)另行安裝。

## 本機重建輸入

建置機仍保留以下檔案，但不提交Git，也不放進本版Release：

| 位置 | 用途與核對方式 |
| --- | --- |
| `vendor/`、`.cargo/config.toml`、`Cargo.lock` | Rust離線來源、固定依賴；空Cargo快取使用`cargo build --frozen`驗證 |
| `offline/python-inputs.zip` | Python重建輸入，整包及各檔雜湊見`offline/python-lock.json` |
| `.build/cad-probe-downloads/` 或 `offline/GstarCAD-Probe.zip` | CAD官方Python／pywin32輸入，URL及SHA256見`tools/gstarcad-probe/dependencies.json` |
| `installer/nsis-3.12.zip` | NSIS建置工具，SHA256固定於`scripts/Build-Installer.ps1` |
| `dist/LM_AI.exe`、NSIS與更新JSON | 本機編譯／封裝產物；EXE仍可供內網部署，不提交Git |

Clone原始碼不等於取得完整離線建置環境。既有建置機的vendor、共用工具鏈及上述輸入保持原位。新建置機請由維護者提供已驗證的離線輸入；也可依鎖定清單從官方來源取得相同檔案並逐檔核對，不可用最新版替代。

Python輸入整包的ZIP中繼資料也會影響整包SHA256，因此自行重新壓縮相同檔案不等於原包。需要新包時必須另行更新鎖定清單並完成runtime重建驗證，不能略過現有整包校驗。CAD可直接將清單中的x64官方檔放入`.build/cad-probe-downloads/`，再執行`tools/build_cad_runtime.py`。

正式建置使用 `scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller`，固定MSVC v142 x64並完成必要測試；Office、CAD、VNC與公司驗收分別記錄。私鑰、Token、設定及測試對話不提交。

## 歷史整理

依使用者授權，main以0.8.75為最早快照，保留後續原始碼變更至0.9.1；刪除保留區間中的發行二進位及建置整包，保留有用素材與驗證記錄。改寫前保存本機Git及LFS備份，不把備份分支推回遠端。既有clone應重新clone，避免將舊歷史推回。

Git引用清理與伺服器儲存回收不同。GitHub指出，刪除LFS引用後物件仍可能計入額度，必要時須聯絡GitHub支援清除。本工具不會為了回收空間刪除整個儲存庫；獨立的NSIS相容性測試分支／Release亦保留。

參考：[GitHub移除LFS檔案說明](https://docs.github.com/en/repositories/working-with-files/managing-large-files/removing-files-from-git-large-file-storage)。實際執行結果見本版驗證文件。
