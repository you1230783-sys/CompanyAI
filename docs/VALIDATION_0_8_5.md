# 0.8.5 供應商與固定安裝路徑

本次交付原始碼、`dist/LM_AI.exe`、`dist/LM_AI_Setup.exe`、EXE／NSIS 配對的更新 JSON 與驗證記錄。不重製離線 ZIP；舊 ZIP 不代表本版原始碼與執行檔。

## 安裝約定

- 供應商／CompanyName 與版權文字改為 `Largan, Inc.`，包含主程式、Setup、Uninstall、已安裝應用程式 Publisher 及「關於」頁面。這是版本資訊，並非 Authenticode 憑證簽章。
- 正式路徑固定 `C:\largan\LM_AI\`；無路徑選擇頁，命令列 `/D` 亦無法覆寫。缺少 `largan` 或 `LM_AI` 時由 NSIS 逐層建立；已存在則沿用。
- 不遞迴刪除目錄或清空未知內容；解除安裝只刪已知程式檔，保留聊天資料與使用者新增的檔案。父目錄或產品目錄是 junction/symlink 時停止。
- 保留 `RequestExecutionLevel user`，目前使用者捷徑與 HKCU 解除安裝登錄；主程式不要求管理員權限。需有固定目錄的寫入權限；若無則明確提示請 IT 配置，不偷偷更換位置或放寬 ACL。
- 舊 `LARGAN.LM_AI` 登錄鍵與單一實例 mutex 是相容識別碼，保留原值以避免重複登錄及破壞更新交接。個人資料仍存於 `%LOCALAPPDATA%\CompanyAI`。

## 舊版與網站部署

若原本安裝在 `%LOCALAPPDATA%\Programs\LM_AI`，建議先從托盤離開，再使用舊版解除安裝，最後安裝 0.8.5；既有解除安裝會保留個人資料。新包不自動刪除舊目錄，直接更新會將捷徑及解除安裝登錄指向新位置，但舊程式檔仍可能存在。遷移後不要再執行舊目錄的解除安裝器，以免它刪除共用的捷徑／登錄。

兩種更新清單同版時仍優先 EXE。要推送 NSIS，網站此次只提供 0.8.5 NSIS 清單；獨立 EXE 清單保留舊版或回 404。完整規則見 `WEB_INTEGRATION.md`。本次只上傳 Git，並未操作公司網站或防毒管理平台。

## 驗證

執行 `scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller`，包括指定 MSVC v142 x64 探針、fmt、Clippy、Rust 測試、Cargo.lock／vendor／.cargo/config.toml 與空 Cargo 快取的 `cargo build --release --frozen`、WebView2 自我檢查，以及主程式驗證兩種更新清單簽章／SHA256。

NSIS 測試包含缺少目錄、沿用既有目錄、忽略 `/D`、捷徑及供應商登錄、等待舊程式退出、更新後重啟、檔案鎖定失敗保留原版、解除安裝保留未知檔案。另實際執行正式 Setup，在 `C:\largan\LM_AI` 安裝真正的 Rust EXE、核對位元組／CompanyName 並執行 WebView2 自我檢查，最後解除安裝。

2026-09-21 本機驗證通過：MSVC 14.29.30133、實際 `_MSC_FULL_VER=192930159` x64，72 項 Rust 測試、空 Cargo 快取的 frozen release 建置、WebView2 自我檢查、EXE／NSIS 清單驗證及上述 NSIS 安裝測試均成功。首次測試前 `C:\largan` 不存在，已實測安裝器建立父／子目錄；正式包另驗證預先存在的產品目錄及其他檔案保留。

初次在受限工具環境的 WebView2 初始化失敗；改在正常 Windows 執行環境重跑完整流程後通過。機器可讀記錄：`offline/exe-verification.json`、`offline/installer-verification.json`、`offline/environment.txt`。公司趨勢防毒白名單效果、實際 Outlook 與首次安裝 WebView2 的公司環境仍需公司驗收；不能以本機測試代替。
