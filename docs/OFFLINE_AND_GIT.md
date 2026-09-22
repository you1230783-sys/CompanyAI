# 離線交付與 Git 工作流程

> 0.8.8 已由使用者實測確認，追加 NSIS 交付：`LM_AI.exe`、`LM_AI_Setup.exe`、EXE／NSIS 更新清單、原始碼與文件，建置為 `Build.ps1 -EmptyCargoCache -IncludeInstaller`。請查看 exe-verification.json、installer-verification.json 與 docs/VALIDATION_0_8_8.md。離線 ZIP／verification.json 仍為歷史版本，未重新製作；vnc-verification.json 保留 0.8.6 的 Viewer 測試，不代表本版重測。

## 專案包含的內容

每個應用程式有自己的資料夾，本專案位於 `Y:\Rust\Project\CompanyAI`。Git 儲存庫應建立在這裡。

```text
CompanyAI/
  src/、examples/           Rust 程式碼與開發範例
  assets/                  可選的 app.ico 與 fallback 說明
  ui/、build.rs             離線前端與資源嵌入；不需要 npm 建置
  Cargo.toml、Cargo.lock    專案與固定依賴
  rust-toolchain.toml       固定 Rust 版本
  .cargo/config.toml       目標平台、靜態 CRT、vendor 設定
  scripts/                 開發機環境、編譯、交付腳本；不在使用者端執行
  installer/               NSIS 安裝原始碼、固定工具 ZIP 與測試 payload
  docs/                    網站串接及離線操作文件
  dist/LM_AI.exe            唯一主程式檔名
  dist/update-manifest-exe.json 獨立 EXE 簽章與下載契約
  dist/LM_AI_Setup.exe      NSIS 完整安裝／更新包
  dist/update-manifest.json 發行包簽章與下載契約
  dist/MicrosoftEdgeWebView2RuntimeInstallerX64.exe  WebView2 離線安裝包
  offline/
    CompanyAI-offline.zip 完整離線包（Git LFS）
    CompanyAI-offline.zip.sha256
    manifest.json         包內原始碼與 EXE 的 SHA256 清單
    verification.json     全新解壓後重新編譯的驗證結果
    environment.txt       實際編譯器版本與 EXE 依賴
  AGENTS.md                給 Codex 的專案規則
  .gitattributes           EXE / ZIP 的 Git LFS 規則
  .gitignore               排除快取、暫存及個人資料
```

離線 ZIP 另外包含 `toolchain/`（Rust、Cargo、rustfmt、Clippy、標準函式庫及 rust-src 原始碼）與 `vendor/`（Cargo.lock 對應的套件原始碼及授權資訊）。
這兩個解壓目錄不另外提交 Git，避免重複保存；所需內容已在 ZIP 內。
ZIP 不包含更新私鑰 .private/、自身、Git 歷史、target、個人登入資料或其他專案的共用服務。

## 拿到公司使用

僅使用程式：複製 `dist\LM_AI.exe`，在 Windows 11 x64 開啟。若缺少 WebView2，先執行 dist 隨附的 x64 離線安裝包，詳見 `dist/WEBVIEW2-OFFLINE.md`。真實服務需要依 `docs/WEB_INTEGRATION.md` 與 `docs/NOTIFICATIONS_AND_OUTLOOK.md` 完成網頁端路由。

需要修改及離線編譯時：

1. 預先確認公司電腦已安裝 **MSVC v142 / 14.29** 與 **Windows SDK 10.0.19041.0**。目前包內不包含 Visual Studio 或 SDK 安裝程式；公司只有其他 14.2x 修補版本或不同 SDK 時，需先核對並調整驗證基準。
2. 將 `offline\CompanyAI-offline.zip` 解壓至新的短路徑，例如 `C:\Work\CompanyAI`。不要在原始碼工作目錄上直接覆蓋解壓，以免蓋掉未交付的修改。
3. 在解壓後的根目錄執行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Build.ps1
```

腳本自動尋找裝有 v142 的 Visual Studio，使用包內 Rust 工具及 vendor 套件，無須安裝 rustup 或接入網際網路。
找不到 Visual Studio 時，可先設定 `$env:RUST_PROJECT_VS_PATH = '實際 Visual Studio 安裝路徑'` 再執行。
驗證包括格式、Clippy、測試、release 編譯、原生視窗自我檢查及 DLL 依賴檢查；完成後更新 `dist\LM_AI.exe`。
測試會使用本機 loopback 模擬 HTTP 服務，不會連線公司 API。

使用 VS Code 編輯時，在解壓後的專案根目錄執行 `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Configure-VSCode.ps1`，再重新載入 VS Code 視窗。
公司電腦需自行準備 VS Code 與 rust-analyzer 擴充套件；本包提供 Rust 開發工具，但不包含編輯器或其擴充套件安裝檔。
`.vscode/settings.json` 含本機工具路徑，不放入 Git / ZIP；搬移或換電腦後重新產生。

## 修改後更新整包內容

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Prepare-Delivery.ps1
```

腳本會整理 vendor、執行完整 Build、建立候選 ZIP，再解壓至暫存目錄。在沒有 target 與 Cargo 快取的狀態，使用 ZIP 內的 Rust 重新完成 Build，通過後才更新固定名稱的 ZIP、校驗及驗證記錄。
ZIP 內有同一份交付腳本，因此公司離線環境也能在修改程式後重新打包。
需要 PowerShell 5.1、Windows 隨附的 tar.exe / robocopy.exe，以及前述 MSVC / SDK。
失敗時會保留暫存目錄以便排查；此時不可提交為完整交付，因為來源或 EXE 可能已更新、ZIP 仍是前次版本。

新增或更新依賴時，先在有網路的開發環境更新 Cargo.lock 並取得 registry 快取，再加上 `-RefreshDependencies` 重建 vendor 與離線包。
因 `.cargo/config.toml` 將 crates.io 替換為 vendor，更新依賴時需暫時停用此 source replacement，完成下載後還原。交付驗證始終使用還原後的設定、固定鎖定檔及 `--frozen`。
一般程式修改不需要重新下載套件。

## 校驗與版本對應

```powershell
Get-FileHash .\offline\CompanyAI-offline.zip -Algorithm SHA256
Get-Content .\offline\CompanyAI-offline.zip.sha256
```

兩個 SHA256 應相同。`verification.json` 的 `archive_sha256` 應同時對應該 ZIP；`manifest.json` 記錄包內原始碼、文件、腳本與 EXE 的雜湊。
文字檔在 Git checkout 後可能受到換行規則影響，逐位元組校驗以 ZIP 解壓內容為準。
ZIP 內不放 ZIP 自身的校驗或解壓驗證結果，避免自我參照；這些記錄與 ZIP 一起提交。
本機通過驗證代表這台電腦以指定編譯器成功建置；公司實機的網路、憑證、登入與服務仍需現場驗收。

## 上傳 Git

遠端儲存庫為 `https://github.com/you1230783-sys/CompanyAI.git`，主要分支為 `main`。使用者已授權首次完整交付上傳；後續收到提交／推送指示時，先更新並驗證交付內容，再提交及推送到此遠端。

首次在本專案初始化儲存庫後、加入 EXE 和 ZIP **之前**，執行：

```powershell
git lfs install --local
```

`.gitattributes` 將 `dist/*.exe` 及 `offline/*.zip` 交由 Git LFS 管理；來源、文件、SHA256 與驗證記錄由一般 Git 保存。
提交前應檢查 `git status`、`git diff --cached --stat` 與 `git lfs ls-files`，確認這次來源及交付檔案一併納入，而且沒有登入憑證或個人設定。
推送後確認遠端分支及 LFS 物件都成功上傳；Git 服務必須支援 LFS，實際額度及限制待選定服務時確認。

在有網路的電腦 clone 後執行 `git lfs pull`，取得真正的 EXE / ZIP 再搬入公司。只有 LFS 文字指標的副本無法執行或解壓。
不要假設 Git 網頁的「下載 ZIP」一定包含 LFS 實體物件；以實際取得檔案及 SHA256 校驗為準。
公司內網若無法連到 Git 服務，可直接搬運本機已下載完成的專案資料夾或離線 ZIP。
