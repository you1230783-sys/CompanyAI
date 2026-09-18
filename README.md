# LARGAN NSIS Probe

獨立的最小 Win32 時鐘與 NSIS 安裝測試，不依賴 Rust、WebView2、Node 或 Python。
程式採 C++ / MSVC v142 x64、靜態 CRT；安裝器採官方 NSIS 3.12 Unicode 標準引擎。
NSIS 官方預編譯安裝器引擎與自行編譯的 x64 小工具是不同的產物。

使用者只需要 `dist` 內的 EXE 與測試說明。兩個 EXE 使用同一份小工具內容，方便比較直接執行與安裝的差異。
目前沒有發行者簽章，不宣稱能通過公司防毒。

## 開發與重建

1. 預先安裝 MSVC v142 x64 與 Windows SDK 10.0.19041.0。
2. 從 https://sourceforge.net/projects/nsis/files/NSIS%203/3.12/ 下載官方 `nsis-3.12.zip`，解壓縮至 `.tools/nsis-3.12`。
3. 開發機執行 `powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/Build.ps1`。

開發腳本會呼叫 vcvars64.bat -vcvars_ver=14.2，再編譯實際檢查 `_MSC_VER` 的程式；這些開發腳本只放在原始碼包，不在使用者的安裝流程執行。
小工具 `--self-check` 會建立隱藏的原生視窗、更新時間控制項並以退出碼回報。
`build/build-report.txt` 記錄實際建置結果，`dist/SHA256SUMS.txt` 記錄 EXE 雜湊。

## 驗證界線

`scripts/Test-Delivery.ps1` 可在開發機實際安裝、檢查登錄／捷徑、啟動已安裝工具的自我檢查，再解除安裝；只操作獨立的 LARGAN_NSIS_Probe 目錄。
公司防毒驗收仍需在目標電腦依 `dist/測試說明.txt` 執行，分別記錄四個階段。

本次本機實測結果存於 `verification/`，安裝及解除安裝均已通過。正式 LM_AI 的來源與產物沒有改動。
GitHub 使用 CompanyAI 儲存庫的獨立 `nsis-probe` 分支與 `nsis-probe-v1.0.0` 測試 Release；不是 LM_AI 新版本。

## NSIS 工具來源

使用 NSIS 3.12 標準發行包，未修改安裝引擎或使用混淆、加殼、停用防毒等手段。
官方 SourceForge 下載在本機回傳網頁，因此取得 MIT MacPorts 鏡像的同一 ZIP 並核對 SHA256：

`56581f90db321581c5381193d796fffcf2d24b2f8fed2160a6c6a3baa67f2c4f`

下載位置：https://mirrors.mit.edu/macports/distfiles/nsis/nsis-3.12.zip
NSIS 原始碼／授權：https://nsis.sourceforge.io/License
